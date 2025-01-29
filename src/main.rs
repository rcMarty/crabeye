mod github;
mod model;

use git2::{Error, Oid, Repository};
use octocrab::{models, params, Octocrab};
use octocrab::models::issues::Issue;
use octocrab::params::State::{Closed};
use secrecy::SecretString;

use Ranal::git;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    log::info!("Hello, world!");

    //init
    let github_repo = github::GitHubApi::new("rust-lang".to_string(), "team".to_string(), load_config())
        .expect("failed to create github repo");
    let repo = git::Repo::init("test_repos/team");


    // pull requests
    let prs = github_repo.get_all_pull_requests(Closed).await?;
    log::info!("PRs: {:#?}", prs.iter().take(5).collect::<Vec<_>>());


    //let commit_id = Oid::from_str("f3569e931be9b92fae2b3237d1073795d753a6f9")?;
    let oid = match prs.first().unwrap().merge_commit_id() {
        Some(oid) => oid,
        None => {
            log::warn!("Cannot find commit id for PR: {:?}", prs.first().unwrap());
            return Ok(());
        }
    };
    let files = repo.modified_files(oid)?.unwrap();
    log::info!("files: {:#?}", files);


    // TODO: wrap pull requests and diff files into one function

    // TODO: save it all to the database

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