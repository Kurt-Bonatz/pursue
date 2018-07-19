use clap::ArgMatches;
use dirs;
use git2::{ErrorCode, Repository, Status, StatusOptions};
use regex::Regex;
use std::env;
use std::fmt;
use tico::tico;

#[cfg(test)]
mod tests;

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
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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
/// well as shorten the directory names if requested.
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

/// Prints out the pre-command line of the prompt.
///
/// This line will be printed in the order of path, git info, and ssh info.
///
/// If the --shorten flag is set however, the non-current directories in the
/// path will be shortened to their first character.
crate fn render(sub_matchings: &ArgMatches) {
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
    }

    println!("{}", precmd);
}
