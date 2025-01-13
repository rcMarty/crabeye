use git2::Repository;
use octocrab::{models, params, Octocrab};
use octocrab::auth::Auth::UserAccessToken;
use secrecy::SecretString;
use serde::Serialize;

#[tokio::main]
async fn main() {
    println!("Hello, world!");
    octocrab_playground().await.expect("octocrab_playground failed");
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

fn git2_playground() {
    let repo = match Repository::open("/path/to/a/repo") {
        Ok(repo) => repo,
        Err(e) => panic!("failed to open: {}", e),
    };
}

async fn octocrab_playground() -> octocrab::Result<()> {
    let login = load_config();
    println!("secret: {:?}", load_config());

    let octocrab = Octocrab::builder().personal_token(login).build()?;
    let octocrab = octocrab::initialise(octocrab);


    let page = octocrab.issues("rust-lang", "team")
        .list()
        .creator("Kobzol")
        .state(params::State::All)
        .per_page(3)
        .send()
        .await?;

    println!("Found {} issues, no of pages: {:?}, total count: {:?}", page.items.len(), page.number_of_pages(), page.total_count);

    let results = octocrab.all_pages::<models::issues::Issue>(page).await?;

    println!("there is overall {} issues", results.len());

    for issue in results.iter().take(5) {
        println!("#{}: {} {}\nauthor(s): {:?}\nlabels: {:?}\n", issue.number, issue.title, issue.body_text.clone().unwrap_or("No body".to_string()), issue.user.login, issue.labels, );
    }


    Ok(())
}