#![allow(unused_imports, unused)]

mod github;
mod model;

use std::collections::HashSet;
use std::fmt::format;
use anyhow::Context;
use git2::{Error, Oid, Repository};
use octocrab::{models, params, Octocrab};
use octocrab::models::issues::Issue;
use octocrab::params::State::{Closed};
use secrecy::SecretString;
use sqlx::sqlite::SqlitePoolOptions;
use ranal::database_connector;
use ranal::database_connector::DatabaseConnection;
use ranal::git;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    log::info!("Hello, world!");

    //init
    let github_repo = github::GitHubApi::new("rust-lang".to_string(), "team".to_string(), load_config())
        .expect("failed to create github repo");
    let repo = git::Repo::init("test_repos/team");


    // pull requests
    let mut prs = github_repo.get_all_pull_requests(Closed).await?;
    log::info!("PRs: {:#?}", prs.iter().take(5).collect::<Vec<_>>());


    // get files from one PR
    //let commit_id = Oid::from_str("f3569e931be9b92fae2b3237d1073795d753a6f9")?;
    let oid = match prs.first().unwrap().merge_commit_id() {
        Some(oid) => oid,
        None => {
            log::warn!("Cannot find commit id for PR: {:?}", prs.first().unwrap());
            return Ok(());
        }
    };
    let files_first = repo.modified_files(oid)?.unwrap_or(HashSet::new());
    log::info!("files: {:#?}", files_first);


    // TODO: wrap pull requests and diff files into one function

    // TODO: save it all to the database

    //TODO do i need to do benchmarks for sql and git2 for files?
    // wouldnt be better to save it straight after fetch to the structure?

    let connection_string = dotenvy::var("DATABASE_URL")
        .context("Cannot load environment variable DATABASE_URL from .env file")?;
    let pool = SqlitePoolOptions::new().connect(&connection_string).await?;
    let db = DatabaseConnection::new(pool).await;

    // edit data to be ready for database insert
    for pr in &prs {
        let oid = match pr.merge_commit_id() {
            Some(oid) => oid,
            None => {
                log::warn!("Cannot find commit id for PR: {:?}", pr);
                continue;
            }
        };
        let files = repo.modified_files(oid)?.unwrap().into_iter().collect();
        log::debug!("files: {:#?}", files);

        // save to database
        db.upsert_user(pr.author.clone()).await?;
        db.save_files_to_pr(pr.pr_number, files).await?;
    }
    log::info!("Data ({}) saved to database", prs.len());


    // retreive all data from database
    let start = std::time::Instant::now();
    let pull_requests = db.get_pull_requests().await;
    let elapsed = start.elapsed();

    log::info!("Retrieved {} pull requests from database in {:?}", pull_requests.len(), elapsed);


    let start2 = std::time::Instant::now();
    for pr in prs.iter_mut() {
        let oid = match pr.merge_commit_id() {
            Some(oid) => oid,
            None => {
                log::warn!("Cannot find commit id for PR: {:?}", pr);
                continue;
            }
        };
        let files = repo.modified_files(oid)?.unwrap().into_iter().collect();
        pr.files = ranal::model::FilesState::Fetched { files };
    }
    let elapsed2 = start2.elapsed();

    log::info!("Retrieved {} pull requests via checking modified files in {:?}", prs.len(), elapsed2);


    //IT IS 8ms VS 700ms for 1600 PRs (700ms was also with console output)


    Ok(())
}

#[derive(serde::Deserialize, Debug)]
struct Config {
    token: Token,
}

#[derive(serde::Deserialize, Debug)]
struct Token {
    secret: String,
}

fn load_config() -> SecretString {
    // get via serde toml
    let config: Config = toml::from_str(include_str!("../config.toml")).expect("failed to parse config");
    SecretString::new(Box::from(config.token.secret))
}

fn git2_playground() -> Result<(), Error> {
    let repo = Repository::open("test_repos/team")?;

    for status in repo.statuses(Some(git2::StatusOptions::new().include_untracked(true)))?.iter() {
        println!("status: {:?}", status.status());
    }

    //get all commit ids
    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;
    revwalk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::TIME)?;

    for id in revwalk.take(10) {
        let id = id?;
        let commit = repo.find_commit(id)?;
        println!("commit: {:?}", commit.message().unwrap());
    }

    //get modified files

    //94cfefb8a926c6f040369b9779ee55248534684d
    //cf72bbeccb270686f411d7bffa2cb24339dd9592
    let oid = Oid::from_str("94cfefb8a926c6f040369b9779ee55248534684d")?;
    let res = repo.find_commit(oid)?;
    println!("commit: {:?}", res.message().unwrap());

    // get information about the commit
    let commit = repo.find_commit(oid)?;
    let diff_tree = commit.tree()?;
    println!("diff tree {:?} with tree id {:?}", diff_tree, commit.tree_id());
    for diff in diff_tree.iter() {
        println!("tree entry name: {:?}", diff.name().unwrap());
        println!("tree entry kind {:?}", diff.kind().unwrap());
    }

    // changed files
    let parent = commit.parent(0)?;
    let diff = repo.diff_tree_to_tree(Some(&parent.tree()?), Some(&commit.tree()?), None)?;
    for delta in diff.deltas() {
        println!("delta: {:?}", delta.new_file().path().unwrap());
    }

    //show canged lines in files
    let diff = repo.diff_tree_to_tree(Some(&parent.tree()?), Some(&commit.tree()?), None)?;
    for delta in diff.deltas() {
        let old_file = delta.old_file();
        let new_file = delta.new_file();
        let old_oid = old_file.id();
        let new_oid = new_file.id();
        let old_blob = repo.find_blob(old_oid)?;
        let new_blob = repo.find_blob(new_oid)?;
        let old_content = String::from_utf8_lossy(old_blob.content());
        let new_content = String::from_utf8_lossy(new_blob.content());
        let old_lines = old_content.lines();
        let new_lines = new_content.lines();

        println!("old content: {:?}", old_content);
        println!("new content: {:?}", new_content);

        for (i, (old_line, new_line)) in old_lines.zip(new_lines).enumerate() {
            if old_line != new_line {
                println!("line {} changed from {} to {}", i + 1, old_line, new_line);
            }
        }
    }

    Ok(())
}

async fn octocrab_playground() -> octocrab::Result<()> {
    let login = load_config();
    println!("secret: {:?}", load_config());

    let octocrab = Octocrab::builder().personal_token(login).build()?;
    let _ = octocrab::initialise(octocrab); // for whatever reason this doesnt return octocrab with auth
    let octocrab = octocrab::instance(); //this is needed to return right octocrab with auth


    let page = octocrab.issues("rust-lang", "team")
        .list()
        .creator("Kobzol")
        .state(params::State::All)
        .per_page(100)
        .send()
        .await?;

    println!("Found {} issues, no of pages: {:?}, total count: {:?}", page.items.len(), page.number_of_pages(), page.total_count);

    // let results = octocrab.all_pages::<models::issues::Issue>(page).await?;

    let result = octocrab.get_page::<Issue>(&page.next).await?;
    println!("result: {:?}", result.clone().unwrap().items.first().unwrap().url);
    let issue = result.unwrap().items;
    // println!("there is overall {} issues", results.len());

    // for issue in results.iter().take(5) {
    //     println!("#{}: {} {}\nauthor(s): {:?}\nlabels: {:?}\n", issue.number, issue.title, issue.body_text.clone().unwrap_or("No body".to_string()), issue.user.login, issue.labels, );
    // }

    for issue in issue {
        println!("#{}: {} {}\nauthor(s): {:?}\nlabels: {:?}\n", issue.number, issue.title, issue.body_text.clone().unwrap_or("No body".to_string()), issue.user.login, issue.labels, );
    }

    let pr = octocrab.pulls("rust-lang", "team")
        .list()
        .state(params::State::All)
        .per_page(100)
        .send()
        .await?;

    let result = octocrab.get_page::<models::pulls::PullRequest>(&pr.next).await?;
    println!("result: {:?}", result.clone().unwrap().items.first().unwrap().url);
    let pr = result.unwrap().items;
    let _ = pr.first().unwrap().changed_files;


    Ok(())
}