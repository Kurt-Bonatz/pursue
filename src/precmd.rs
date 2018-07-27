use clap::ArgMatches;
use dirs;
use git2::{ErrorCode, Repository, Status, StatusOptions};
use regex::Regex;
use std::env;
use std::fmt;
use tico::tico;

#[derive(Debug)]
struct PrePrompt {
    path: String,
    user_name: String,
    host: String,
    vcs_branch: String,
    vcs_is_dirty: bool,
    vcs_is_behind_remote: bool,
    vcs_is_ahead_of_remote: bool,
}

impl fmt::Display for PrePrompt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Always write the path
        write!(f, "{}", self.path)?;

        // Write out the branch name if we are in a VCS directory.
        if !self.vcs_branch.is_empty() {
            write!(f, " {}", self.vcs_branch)?;

            // Print a star if the working directory is dirty.
            if self.vcs_is_dirty {
                write!(f, "*")?;
            }

            // Print arrows corresponding to whether or not we are out of date
            // or ahead of the branch's remote.
            if self.vcs_is_behind_remote {
                write!(f, "⭭")?;
            }

            if self.vcs_is_ahead_of_remote {
                write!(f, "⭫")?;
            }
        }

        // Write out the host name in the form user@host if both are set.
        if !self.user_name.is_empty() && !self.host.is_empty() {
            write!(f, " {}@{}", self.user_name, self.host)?;
        }

        Ok(())
    }
}

/// Formats the current path to replace the path of HOME with the usual '~' as
/// well as sortens the directory names if requested.
fn format_path(cwd: &str, home_dir: &str, shorten: bool) -> String {
    let path = Regex::new(home_dir).unwrap().replace(cwd, "~");

    if shorten {
        return tico(&path);
    }

    String::from(path)
}

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

fn is_dirty(repo: &Repository) -> bool {
    let mut options = StatusOptions::new();
    options.include_untracked(true);

    let statuses = match repo.statuses(Some(&mut options)) {
        Ok(statuses) => statuses,
        Err(_) => return false,
    };

    let mut clean_status = Status::empty();
    clean_status.toggle(Status::CURRENT);
    clean_status.toggle(Status::IGNORED);
    statuses
        .iter()
        .any(|entry| !entry.status().is_empty() && !entry.status().intersects(clean_status))
}

/// Determine if the current HEAD is ahead/behind its remote. The tuple
/// returned will be in the order ahead and then behind.
///
/// If the remote is not set or doesn't exit (like a detached HEAD),
/// (false, false) will be returned.
fn is_ahead_behind_remote(repo: &Repository) -> (bool, bool) {
    let head = repo.revparse_single("HEAD").unwrap().id();
    if let Some((upstream, _)) = repo.revparse_ext("@{u}").ok() {
        return match repo.graph_ahead_behind(head, upstream.id()) {
            Ok((commits_ahead, commits_behind)) => (commits_ahead > 0, commits_behind > 0),
            Err(_) => (false, false),
        };
    }
    (false, false)
}

/// Prints out the pre-command line of the prompt.
///
/// This line will be printed in the order of path, git info, and ssh info.
///
/// If the --shorten flag is set however, the non-current directories in the
/// path will be shortened to their first character.
crate fn render(sub_matchings: &ArgMatches<'_>) {
    let mut precmd = PrePrompt {
        path: String::from(""),
        user_name: String::from(""),
        host: String::from(""),
        vcs_branch: String::from(""),
        vcs_is_dirty: false,
        vcs_is_behind_remote: false,
        vcs_is_ahead_of_remote: false,
    };

    let shorten = sub_matchings.is_present("shorten");
    let working_dir = env::current_dir().unwrap();
    let home_dir = match dirs::home_dir() {
        Some(dir) => String::from(dir.to_str().unwrap()),
        _ => String::from(""),
    };
    precmd.path = format_path(working_dir.to_str().unwrap(), &home_dir, shorten);

    if let Some(repo) = Repository::discover(".").ok() {
        precmd.vcs_branch = branch_name(&repo);
        precmd.vcs_is_dirty = is_dirty(&repo);

        let (ahead, behind) = is_ahead_behind_remote(&repo);
        precmd.vcs_is_ahead_of_remote = ahead;
        precmd.vcs_is_behind_remote = behind;
    }

    println!("{}", precmd);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::path::Path;
    use tempdir::TempDir;

    #[test]
    fn pre_prompt_only_path_prints_just_the_path() {
        let precmd = PrePrompt {
            path: String::from("~/some/dir"),
            user_name: String::from(""),
            host: String::from(""),
            vcs_branch: String::from(""),
            vcs_is_dirty: false,
            vcs_is_behind_remote: false,
            vcs_is_ahead_of_remote: false,
        };

        assert_eq!("~/some/dir".to_owned(), format!("{}", precmd));
    }

    #[test]
    fn pre_prompt_prints_just_path_when_only_has_username() {
        let precmd = PrePrompt {
            path: String::from("~/some/dir"),
            user_name: String::from("user_name"),
            host: String::from(""),
            vcs_branch: String::from(""),
            vcs_is_dirty: false,
            vcs_is_behind_remote: false,
            vcs_is_ahead_of_remote: false,
        };

        assert_eq!("~/some/dir".to_owned(), format!("{}", precmd));
    }

    #[test]
    fn pre_prompt_prints_user_name_and_host() {
        let precmd = PrePrompt {
            path: String::from("~"),
            user_name: String::from("user"),
            host: String::from("host"),
            vcs_branch: String::from(""),
            vcs_is_dirty: false,
            vcs_is_behind_remote: false,
            vcs_is_ahead_of_remote: false,
        };

        assert_eq!("~ user@host".to_owned(), format!("{}", precmd));
    }

    #[test]
    fn pre_prompt_prints_branch_name() {
        let precmd = PrePrompt {
            path: String::from("~"),
            user_name: String::from("user"),
            host: String::from("host"),
            vcs_branch: String::from("master"),
            vcs_is_dirty: false,
            vcs_is_behind_remote: false,
            vcs_is_ahead_of_remote: false,
        };

        assert_eq!("~ master user@host".to_owned(), format!("{}", precmd));
    }

    #[test]
    fn pre_prompt_prints_dirty() {
        let precmd = PrePrompt {
            path: String::from("~"),
            user_name: String::from("user"),
            host: String::from("host"),
            vcs_branch: String::from("master"),
            vcs_is_dirty: true,
            vcs_is_behind_remote: false,
            vcs_is_ahead_of_remote: false,
        };

        assert_eq!("~ master* user@host".to_owned(), format!("{}", precmd));
    }

    #[test]
    fn pre_prompt_prints_dirty_upstream_downstream() {
        let precmd = PrePrompt {
            path: String::from("~"),
            user_name: String::from("user"),
            host: String::from("host"),
            vcs_branch: String::from("master"),
            vcs_is_dirty: true,
            vcs_is_behind_remote: true,
            vcs_is_ahead_of_remote: true,
        };

        assert_eq!(
            "~ master*⭭⭫ user@host".to_owned(),
            format!("{}", precmd)
        );
    }

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
        let branch =
            repo.branch(
                "test_branch",
                &repo.head().unwrap().peel_to_commit().unwrap(),
                false,
            ).unwrap();
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
        let mut local =
            repo.branch(
                "local_branch",
                &repo.head().unwrap().peel_to_commit().unwrap(),
                false,
            ).unwrap();

        // Make a "remote" branch
        repo.branch(
            "remote_branch",
            &repo.head().unwrap().peel_to_commit().unwrap(),
            false,
        ).unwrap();
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
