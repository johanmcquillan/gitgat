extern crate git2;

use std::cmp;
use std::time::Duration;

use git2::{DiffDelta, DiffLine, DiffOptions, Oid, Repository, Sort};
use indicatif::{ProgressBar, ProgressIterator, ProgressStyle};

/// Options for running gitgat.
pub struct Opts<'a> {
    pub repo: &'a str,
    pub author: &'a str,
    pub excluded_dirs: Vec<&'a str>,
}

#[derive(Default)]
struct Commit {
    hash: String,
    summary: String,
    additions: u32,
    deletions: u32,
}

impl Commit {
    fn new(hash: String, summary: String) -> Commit {
        Commit {
            hash: hash,
            summary: summary,
            additions: 0,
            deletions: 0,
        }
    }

    fn new_from_commit(c: git2::Commit) -> Commit {
        Commit::new(
            c.id().to_string().to_owned(),
            c.summary().unwrap().to_owned(),
        )
    }
    fn size(&self) -> u32 {
        cmp::max(self.additions, self.deletions)
    }
}

#[derive(Default)]
struct Stats<'a> {
    commits: u32,
    additions: u32,
    deletions: u32,
    top: Option<&'a Commit>,
}

impl<'a> Stats<'a> {
    fn update(mut self, commit: &'a Commit) -> Stats<'a> {
        self.commits += 1;
        self.additions += commit.additions;
        self.deletions += commit.deletions;
        match self.top {
            Some(top) => {
                if commit.size() > top.size() {
                    self.top = Some(commit)
                }
            }
            None => self.top = Some(commit),
        };
        return self;
    }
}

#[derive(Default)]
struct History {
    commits: Vec<Commit>,
}

impl<'a> History {
    fn stats(&'a self) -> Stats<'a> {
        self.commits.iter().fold(Stats::default(), Stats::update)
    }
}

/// Run gitgat on a repository.
pub fn run(opts: Opts) {
    let repo = match Repository::open(opts.repo) {
        Ok(repo) => repo,
        Err(e) => panic!("failed to open: {}", e),
    };

    let oids = collect_oids(&repo);

    let mut history = History::default();

    for i in (0..oids.len()).progress_with_style(oid_progress_style()) {
        let commit = repo.find_commit(oids[i]).unwrap();
        if commit.author().name() != Some(opts.author) {
            continue;
        }
        let prev_commit = repo.find_commit(oids[i + 1]).unwrap();
        let diff = repo
            .diff_tree_to_tree(
                Some(&prev_commit.tree().unwrap()),
                Some(&commit.tree().unwrap()),
                Some(
                    &mut DiffOptions::default()
                        .ignore_blank_lines(true)
                        .ignore_filemode(true),
                ),
            )
            .unwrap();

        let mut c = Commit::new_from_commit(commit);
        diff.foreach(
            &mut (|_, _| true),
            None,
            None,
            Some(&mut |delta: DiffDelta, _, line: DiffLine| -> bool {
                // Skip if the line if it is in an excluded directory.
                if opts
                    .excluded_dirs
                    .iter()
                    .any(|dir| delta.new_file().path().unwrap().starts_with(dir))
                {
                    return true;
                };
                match line.origin() {
                    '+' => c.additions += 1,
                    '-' => c.deletions += 1,
                    _ => {}
                };
                return true;
            }),
        )
        .unwrap();
        history.commits.push(c);
    }
    let stats = history.stats();
    println!(" {} commits", stats.commits);
    println!("+{}", stats.additions);
    println!("-{}", stats.deletions);
    println!("Biggest commit {}", &stats.top.unwrap().hash);
    println!("Biggest commit {}", &stats.top.unwrap().size());
    println!("Biggest commit {}", &stats.top.unwrap().summary);
}

fn oid_progress_style() -> ProgressStyle {
    ProgressStyle::with_template(
        "[{elapsed_precise}] [{bar:30.green}] {human_pos:>7}/{human_len:7} commits",
    )
    .unwrap()
    .progress_chars("▮ ")
}

/// Extracts a vector of object IDs from repository.
fn collect_oids(repo: &Repository) -> Vec<Oid> {
    let mut revwalk = repo.revwalk().unwrap();
    revwalk.push_head().unwrap();
    revwalk.set_sorting(Sort::TOPOLOGICAL).unwrap();
    let collector_pb = ProgressBar::new_spinner().with_style(
        ProgressStyle::with_template("Collecting commits {spinner}")
            .unwrap()
            .tick_chars("▖▘▝▗"),
    );
    collector_pb.enable_steady_tick(Duration::from_millis(500));
    let oids: Vec<Oid> = revwalk.map(|o| o.unwrap()).collect();
    collector_pb.disable_steady_tick();
    collector_pb.is_finished();

    return oids;
}
