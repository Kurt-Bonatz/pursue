#[cfg(test)]
use super::*;
use git2::ObjectType;
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
    repo.checkout_tree(&branch.get().peel(ObjectType::Tree).unwrap(), None)
        .unwrap();
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

fn temp_repo() -> (TempDir, Repository) {
    let dir = TempDir::new("repo").unwrap();
    let repo = Repository::init(dir.path()).unwrap();
    (dir, repo)
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
