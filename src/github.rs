use octocrab::{params, Octocrab};
use octocrab::models::issues::Issue;
use secrecy::SecretString;
use ::Ranal::model::PullRequest;
use Ranal::model::FilesState;

pub struct GitHubApi {
    owner: String,
    repository: String,
    octocrab: Octocrab,
}

impl GitHubApi {
    /// Create a new GitHubApi instance
    /// * token - GitHub personal access token
    pub fn new(owner: String, repository: String, token: SecretString) -> anyhow::Result<Self> {
        let octocrab = Octocrab::builder().personal_token(token).build()?;
        // let _ = octocrab::initialise(octocrab); // for whatever reason this doesnt return octocrab with auth
        // let octocrab = octocrab::instance(); //this is needed to return right octocrab with auth

        Ok(Self {
            owner,
            repository,
            octocrab,
        })
    }

    pub async fn get_issues(&self) -> anyhow::Result<Vec<Issue>> {
        let page = self.octocrab.issues(self.owner.clone(), self.repository.clone())
            .list()
            .creator("Kobzol")
            .state(params::State::All)
            .per_page(100)
            .send()
            .await?;

        println!("Found {} issues, no of pages: {:?}, total count: {:?}", page.items.len(), page.number_of_pages(), page.total_count);

        // let results = octocrab.all_pages::<models::issues::Issue>(page).await?;

        let result = self.octocrab.get_page::<Issue>(&page.next).await?;
        println!("result: {:?}", result.clone().unwrap().items.first().unwrap().url);
        let issues = result.unwrap().items;
        // println!("there is overall {} issues", results.len());

        // for issue in results.iter().take(5) {
        //     println!("#{}: {} {}\nauthor(s): {:?}\nlabels: {:?}\n", issue.number, issue.title, issue.body_text.clone().unwrap_or("No body".to_string()), issue.user.login, issue.labels, );
        // }

        for issue in issues.iter() {
            println!("#{}: {} {}\nauthor(s): {:?}\nlabels: {:?}\n", issue.number, issue.title, issue.body_text.clone().unwrap_or("No body".to_string()), issue.user.login, issue.labels, );
        }
        Ok(issues)
    }

    pub async fn get_pull_requests(&self, state: params::State) -> anyhow::Result<Vec<PullRequest>> {
        let pr = self.octocrab.pulls(self.owner.clone(), self.repository.clone())
            .list()
            .state(state)
            .per_page(100)
            .send()
            .await?;

        println!("Found {} pull requests, no of pages: {:?}", pr.items.len(), pr.number_of_pages());

        // let result = self.octocrab.get_page::<octocrab::models::pulls::PullRequest>(&pr.next).await?;
        // println!("result: {:?}", result.clone().unwrap().items.first().unwrap().url);
        // let pr = result.unwrap().items;

        let pr = self.octocrab.all_pages::<octocrab::models::pulls::PullRequest>(pr).await?;

        let mut parsed_prs: Vec<PullRequest> = Vec::new();
        for pr in pr {
            let parsed = PullRequest {
                commit_id: pr.head.sha.clone(),
                title: pr.title.clone(),
                author: pr.user.unwrap().login,
                state: pr.state,
                description: pr.body.clone(),
                created_at: pr.created_at,
                updated_at: pr.updated_at,
                files: FilesState::NotFetched,
            };
            parsed_prs.push(parsed);
        }

        Ok(parsed_prs)
    }
}