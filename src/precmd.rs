use ansi_term::Colour::{Blue, Cyan, Fixed, White};
use clap::ArgMatches;
use dirs;
use git2::{ErrorCode, Repository, Status, StatusOptions};
use regex::Regex;
use std::env;
use std::fmt;
use std::fs::File;
use std::io::prelude::*;
use std::process::Command;
use tico::tico;

#[derive(Debug, PartialEq)]
struct SshInfo {
    user: String,
    is_root: bool,
    host: String,
}

impl fmt::Display for SshInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let gray = Fixed(242);
        if !self.is_root {
            write!(f, "{}", gray.paint(format!("{}@{}", self.user, self.host)))
        } else {
            write!(
                f,
                "{}{}",
                White.paint(&self.user),
                gray.paint(format!("@{}", self.host))
            )
        }
    }
}

#[derive(Debug, PartialEq)]
struct VcsInfo {
    branch: String,
    is_dirty: bool,
    is_behind_remote: bool,
    is_ahead_of_remote: bool,
}

impl fmt::Display for VcsInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let gray = Fixed(242);
        let mut branch = self.branch.to_owned();

        // Print a star if the working directory is dirty.
        if self.is_dirty {
            branch.push('*')
        }
        write!(f, "{}", gray.paint(branch))?;

        match (self.is_behind_remote, self.is_ahead_of_remote) {
            (true, true) => write!(f, " {}", Cyan.paint("⇣⇡")),
            (true, false) => write!(f, " {}", Cyan.paint("⇣")),
            (false, true) => write!(f, " {}", Cyan.paint("⇡")),
            _ => Ok(()),
        }
    }
}

#[derive(Debug, PartialEq)]
struct PrePrompt {
    path: String,
    vcs_info: Option<VcsInfo>,
    ssh_info: Option<SshInfo>,
}

impl PrePrompt {
    fn new() -> PrePrompt {
        PrePrompt {
            path: String::new(),
            vcs_info: None,
            ssh_info: None,
        }
    }
}

impl fmt::Display for PrePrompt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Always write the path
        write!(f, "{}", Blue.paint(&self.path))?;

        // Write out the vcs info if we have it.
        if let Some(vcs) = &self.vcs_info {
            write!(f, " {}", vcs)?;
        }

        // Write out the user and host name if we have them.
        if let Some(info) = &self.ssh_info {
            write!(f, " {}", info)?;
        }

        Ok(())
    }
}

/// Formats the current path to replace the path of HOME with the usual '~' as well as sortens the
/// directory names if requested.
fn format_path(cwd: &str, home_dir: &str, shorten: bool) -> String {
    let path = Regex::new(home_dir).unwrap().replace(cwd, "~");

    if shorten {
        return tico(&path);
    }

    String::from(path)
}

/// Checks if there ssh connection, and returns `SshInfo` if the username and host name of the
/// remote session are available.
fn get_ssh_info() -> Option<SshInfo> {
    // $HOSTNAME isn't a posix defined environment variable and sometimes doesn't exist when called
    // from a new `sh` process instead of `bash` or `zsh` where it is often predefined. In order to
    // still get the hostname, we'll just parse it directly from the hostname file.
    let mut file = File::open("/etc/hostname").ok()?;
    let mut host = String::new();
    file.read_to_string(&mut host).ok()?;

    match (
        env::var("SSH_CONNECTION"),
        env::var("USER"),
        env::var("UID"),
    ) {
        (Ok(_), Ok(user), Ok(uid)) => Some(SshInfo {
            user,
            is_root: uid == "0",
            host,
        }),
        _ => None,
    }
}

/// Finds the current branch name.
///
/// If the repository was just initialized and doesn't have any commits, "master" is returned.
/// Otherwise any failure returns an empty String.
fn branch_name(repo: &Repository) -> String {
    match repo.head() {
        Ok(head) => head.shorthand().unwrap().to_string(),
        Err(e) => {
            // In a new repo with no commits, HEAD points to a branch with no
            // commits. So let's just call the branch 'master'.
            if e.code() == ErrorCode::UnbornBranch {
                String::from("master")
            } else {
                String::from("")
            }
        }
    }
}

/// Determines if the repository is in a dirty state, ignoring any files listed in .gitignore.
/// Untracked files are also considered a dirty state.
fn is_dirty() -> bool {
    // Note: we use a subprocess here since the underlying libgit2 library's git status call is
    // extremely slow (6x as slow in some cases). This appears to be due to the fact that it is not
    // multithreaded and unfortunately git2-rs only implements Send for the Repository object
    // effectively making multithreaded status checks over all files in the tree slower than a
    // single thread dealing with context switching and one mutex across all threads.
    match Command::new("git").args(&["diff", "--no-ext-diff", "--quiet", "--exit-code"]).status() {
        Ok(code) => !code.success(),
        Err(_) => false
    }
}

/// Determine if the current HEAD is ahead/behind its remote. The tuple returned will be in the
/// order ahead and then behind.
///
/// If the remote is not set or doesn't exit (like a detached HEAD), (false, false) will be
/// returned.
fn is_ahead_behind_remote(repo: &Repository) -> (bool, bool) {
    let head = repo.revparse_single("HEAD").unwrap().id();
    repo.revparse_ext("@{u}")
        .ok()
        .and_then(|(upstream, _)| repo.graph_ahead_behind(head, upstream.id()).ok())
        .map(|(ahead, behind)| (ahead > 0, behind > 0))
        .unwrap_or((false, false))
}

/// Prints out the pre-command line of the prompt.
///
/// This line will be printed in the order of path, git info, and ssh info.
///
/// If the --shorten flag is set however, the non-current directories in the path will be shortened
/// to their first character.
crate fn render(sub_matchings: &ArgMatches<'_>) {
    let mut precmd = PrePrompt::new();

    let shorten = sub_matchings.is_present("shorten");
    let working_dir = env::current_dir().unwrap();
    let home_dir = match dirs::home_dir() {
        Some(dir) => String::from(dir.to_str().unwrap()),
        _ => String::from(""),
    };
    precmd.path = format_path(working_dir.to_str().unwrap(), &home_dir, shorten);
    precmd.ssh_info = get_ssh_info();

    if let Ok(repo) = Repository::discover(".") {
        let (ahead, behind) = is_ahead_behind_remote(&repo);
        precmd.vcs_info = Some(VcsInfo {
            branch: branch_name(&repo),
            is_dirty: is_dirty(),
            is_behind_remote: behind,
            is_ahead_of_remote: ahead,
        });
    }

    println!("{}", precmd);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::path::Path;
    use tempdir::TempDir;

    ////////////////////////////////////////////////////////////////////////////////
    // PrePrompt formatting tests
    ////////////////////////////////////////////////////////////////////////////////

    #[test]
    fn pre_prompt_only_path_prints_just_the_path() {
        let mut precmd = PrePrompt::new();
        precmd.path = String::from("~/some/dir");
        assert_eq!(
            format!("{}", Blue.paint("~/some/dir")),
            format!("{}", precmd)
        );
    }

    #[test]
    fn pre_prompt_prints_user_name_and_host() {
        let mut precmd = PrePrompt::new();
        precmd.path = String::from("~");
        precmd.ssh_info = Some(SshInfo {
            user: String::from("user"),
            is_root: false,
            host: String::from("host"),
        });

        assert_eq!(
            format!("{} {}", Blue.paint("~"), Fixed(242).paint("user@host")),
            format!("{}", precmd)
        );
    }

    #[test]
    fn pre_prompt_prints_branch_name() {
        let mut precmd = PrePrompt::new();
        precmd.path = String::from("~");
        precmd.ssh_info = Some(SshInfo {
            user: String::from("user"),
            is_root: false,
            host: String::from("host"),
        });
        precmd.vcs_info = Some(VcsInfo {
            branch: String::from("master"),
            is_dirty: false,
            is_behind_remote: false,
            is_ahead_of_remote: false,
        });

        assert_eq!(
            format!(
                "{} {} {}",
                Blue.paint("~"),
                Fixed(242).paint("master"),
                Fixed(242).paint("user@host")
            ),
            format!("{}", precmd)
        );
    }

    ////////////////////////////////////////////////////////////////////////////////
    // SshInfo formatting tests
    ////////////////////////////////////////////////////////////////////////////////

    #[test]
    fn ssh_info_prints_root_as_white() {
        let ssh_info = SshInfo {
            user: String::from("user"),
            is_root: true,
            host: String::from("host"),
        };

        assert_eq!(
            format!("{}{}", White.paint("user"), Fixed(242).paint("@host")),
            format!("{}", ssh_info)
        );
    }

    ////////////////////////////////////////////////////////////////////////////////
    // VcsInfo formatting tests
    ////////////////////////////////////////////////////////////////////////////////

    #[test]
    fn vcs_info_prints_dirty_repo() {
        let vcs = VcsInfo {
            branch: String::from("master"),
            is_dirty: true,
            is_behind_remote: false,
            is_ahead_of_remote: false,
        };

        assert_eq!(
            format!("{}", Fixed(242).paint("master*")),
            format!("{}", vcs)
        );
    }

    #[test]
    fn vcs_info_prints_behind_remote() {
        let vcs = VcsInfo {
            branch: String::from("master"),
            is_dirty: false,
            is_behind_remote: true,
            is_ahead_of_remote: false,
        };

        assert_eq!(
            format!("{} {}", Fixed(242).paint("master"), Cyan.paint("⇣")),
            format!("{}", vcs)
        );
    }

    #[test]
    fn vcs_info_prints_ahead_of_remote() {
        let vcs = VcsInfo {
            branch: String::from("master"),
            is_dirty: false,
            is_behind_remote: false,
            is_ahead_of_remote: true,
        };

        assert_eq!(
            format!("{} {}", Fixed(242).paint("master"), Cyan.paint("⇡")),
            format!("{}", vcs)
        );
    }

    #[test]
    fn pre_prompt_prints_dirty_upstream_downstream() {
        let vcs = VcsInfo {
            branch: String::from("master"),
            is_dirty: true,
            is_behind_remote: true,
            is_ahead_of_remote: true,
        };

        assert_eq!(
            format!("{} {}", Fixed(242).paint("master*"), Cyan.paint("⇣⇡")),
            format!("{}", vcs)
        );
    }

    ////////////////////////////////////////////////////////////////////////////////
    // Path name tests
    ////////////////////////////////////////////////////////////////////////////////

    #[test]
    fn format_path_home_is_shortened() {
        let home = format_path("/home/user", "/home/user", false);
        assert_eq!(home, "~", "Home path {} wasn't shortened to '~'!", home);

        let path = format_path("home/user/pursue/src", "home/user", false);
        assert_eq!(path, "~/pursue/src");
    }

    #[test]
    fn format_path_non_current_directories_are_shortened() {
        let long = format_path("home/user/Really/long/path", "home/user", true);
        assert_eq!(long, "~/R/l/path");
    }

    ////////////////////////////////////////////////////////////////////////////////
    // SSH info tests
    ////////////////////////////////////////////////////////////////////////////////

    #[test]
    fn ssh_info_returns_basic_user_host_info() {
        // Set up our env variables
        if env::var("SSH_CONNECTION").is_err() {
            env::set_var("SSH_CONNECTION", "pursue_test_val");
        }
        env::set_var("USER", "pursue");
        env::set_var("UID", "1000");

        if let Some(host) = get_hostname() {
            assert_eq!(
                get_ssh_info().unwrap(),
                SshInfo { user: String::from("pursue"), is_root: false, host }
            );
        } else {
            assert!(get_ssh_info().is_none());
        }
    }

    // These are currently super flaky since cargo test operates in parallel and we
    // are changing environment variables and expect a serial environment.
    //
    // #[test]
    // fn ssh_info_returns_none_for_no_ssh_connection() {
    //     env::remove_var("SSH_CONNECTION");
    //     assert!(get_ssh_info().is_none());
    // }

    // #[test]
    // fn ssh_info_detects_root() {
    //     if env::var("SSH_CONNECTION").is_err() {
    //         env::set_var("SSH_CONNECTION", "pursue_test_val");
    //     }
    //     env::set_var("USER", "pursue");
    //     env::set_var("UID", "0");
    //     assert_eq!(env::var("UID"), Ok("0".to_string()));

    //     if let Some(host) = get_hostname() {
    //         assert_eq!(
    //             get_ssh_info().unwrap(),
    //             SshInfo { user: String::from("pursue"), is_root: true, host }
    //         );
    //     } else {
    //         assert!(get_ssh_info().is_none());
    //     }
    // }

    ////////////////////////////////////////////////////////////////////////////////
    // Git tests
    ////////////////////////////////////////////////////////////////////////////////

    #[test]
    fn branch_name_uses_master_with_brand_new_repo() {
        let (_td, repo) = temp_repo();
        let branch = branch_name(&repo);
        assert_eq!(branch, "master");
    }

    #[test]
    fn branch_name_returns_correct_name() {
        let (_td, repo) = temp_repo();
        init_repo(&repo);
        let branch = repo
            .branch(
                "test_branch",
                &repo.head().unwrap().peel_to_commit().unwrap(),
                false,
            )
            .unwrap();
        repo.set_head("refs/heads/test_branch").unwrap();
        repo.checkout_head(None).unwrap();
        assert!(branch.is_head());

        let branch = branch_name(&repo);
        assert_eq!(branch, "test_branch");
    }

    #[test]
    fn is_dirty_with_untracked_change() {
        let (_td, repo) = temp_repo();
        init_repo(&repo);

        let root = repo.path().parent().unwrap();
        File::create(&root.join("unstaged_file")).unwrap();

        assert!(is_dirty(&repo));
    }

    #[test]
    fn is_dirty_with_unstaged_change() {
        let (_td, repo) = temp_repo();
        init_repo(&repo);
        let mut index = repo.index().unwrap();

        let root = repo.path().parent().unwrap();
        File::create(&root.join("unstaged_file")).unwrap();
        index.add_path(Path::new("unstaged_file")).unwrap();
        index.write().unwrap();

        assert!(is_dirty(&repo));
    }

    #[test]
    fn unset_remote_is_not_ahead_or_behind() {
        let (_td, repo) = temp_repo();
        init_repo(&repo);
        assert_eq!(is_ahead_behind_remote(&repo), (false, false));
    }

    #[test]
    fn is_ahead_and_behind_remote() {
        let (_td, repo) = temp_repo();
        init_repo(&repo);

        // Make a "local" branch
        let mut local = repo
            .branch(
                "local_branch",
                &repo.head().unwrap().peel_to_commit().unwrap(),
                false,
            )
            .unwrap();

        // Make a "remote" branch
        repo.branch(
            "remote_branch",
            &repo.head().unwrap().peel_to_commit().unwrap(),
            false,
        )
        .unwrap();
        repo.set_head("refs/heads/remote_branch").unwrap();
        repo.checkout_head(None).unwrap();

        // Make a second commit on the remote branch
        make_commit(&repo, "remote_file", "remote commit");

        repo.set_head("refs/heads/local_branch").unwrap();
        repo.checkout_head(None).unwrap();
        assert_eq!(is_ahead_behind_remote(&repo), (false, false));

        // Track the "remote" branch which should be ahead of the "local" branch
        local.set_upstream(Some("remote_branch")).unwrap();
        assert_eq!(is_ahead_behind_remote(&repo), (false, true));

        // Make a second commit on the local branch
        make_commit(&repo, "local_file", "local commit");

        // The "local" branch should now have a commit ahead of the "remote"
        assert_eq!(is_ahead_behind_remote(&repo), (true, true));
    }

    ////////////////////////////////////////////////////////////////////////////////
    // Helper methods
    ////////////////////////////////////////////////////////////////////////////////

    fn get_hostname() -> Option<String> {
        let mut file = File::open("/etc/hostname").ok()?;
        let mut host = String::new();
        file.read_to_string(&mut host).ok()?;
        Some(host)
    }

    fn temp_repo() -> (TempDir, Repository) {
        let dir = TempDir::new("repo").unwrap();
        let repo = Repository::init(dir.path()).unwrap();
        (dir, repo)
    }

    fn make_commit(repo: &Repository, file_name: &str, msg: &str) {
        let mut index = repo.index().unwrap();
        let id = index.write_tree().unwrap();
        let tree = repo.find_tree(id).unwrap();
        let sig = repo.signature().unwrap();
        let root = repo.path().parent().unwrap();
        let head = repo.head().unwrap();
        let target = head.target().unwrap();
        let commit = repo.find_commit(target).unwrap();

        File::create(&root.join(file_name)).unwrap();
        index.add_path(Path::new(file_name)).unwrap();
        index.add_path(Path::new(file_name)).unwrap();
        index.write().unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, msg, &tree, &[&commit])
            .unwrap();
    }

    fn init_repo(repo: &Repository) {
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "name").unwrap();
        config.set_str("user.email", "email").unwrap();
        let mut index = repo.index().unwrap();
        let id = index.write_tree().unwrap();

        let tree = repo.find_tree(id).unwrap();
        let sig = repo.signature().unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "initial", &tree, &[])
            .unwrap();
    }
}
