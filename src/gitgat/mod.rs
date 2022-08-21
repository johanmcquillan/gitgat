extern crate git2;

use std::cmp;
use std::error;
use std::fmt;
use std::time;

use indicatif::{ProgressBar, ProgressIterator, ProgressStyle};

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    Git(git2::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            // The wrapped error contains additional information and is available
            // via the source() method.
            Error::Git(err) => write!(f, "encountered a git error: {}", err),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match *self {
            Error::Git(ref e) => Some(e),
        }
    }
}

impl From<git2::Error> for Error {
    fn from(err: git2::Error) -> Error {
        Error::Git(err)
    }
}

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
            c.summary().unwrap_or("<unknown summary>").to_owned(),
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
pub fn run(opts: Opts) -> Result<()> {
    let repo = git2::Repository::open(opts.repo)?;
    let oids = collect_oids(&repo)?;

    let mut history = History::default();
    for i in (0..oids.len()).progress_with_style(oid_progress_style()) {
        let commit = repo.find_commit(oids[i])?;
        if commit.author().name() != Some(opts.author) {
            continue;
        }
        let prev_commit = repo.find_commit(oids[i + 1])?;
        let diff = repo.diff_tree_to_tree(
            Some(&prev_commit.tree()?),
            Some(&commit.tree()?),
            Some(
                &mut git2::DiffOptions::default()
                    .ignore_blank_lines(true)
                    .ignore_filemode(true),
            ),
        )?;

        let mut c = Commit::new_from_commit(commit);
        diff.foreach(
            &mut (|_, _| true),
            None,
            None,
            Some(
                &mut |delta: git2::DiffDelta, _, line: git2::DiffLine| -> bool {
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
                },
            ),
        )?;
        history.commits.push(c);
    }
    let stats = history.stats();
    println!(" {} commits", stats.commits);
    println!("+{}", stats.additions);
    println!("-{}", stats.deletions);
    println!("Biggest commit {}", &stats.top.unwrap().hash);
    println!("Biggest commit {}", &stats.top.unwrap().size());
    println!("Biggest commit {}", &stats.top.unwrap().summary);
    Ok(())
}

fn oid_progress_style() -> ProgressStyle {
    ProgressStyle::with_template(
        "[{elapsed_precise}] [{bar:30.green}] {human_pos:>7}/{human_len:7} commits",
    )
    .unwrap()
    .progress_chars("▮ ")
}

/// Extracts a vector of object IDs from repository.
fn collect_oids(repo: &git2::Repository) -> Result<Vec<git2::Oid>> {
    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;
    revwalk.set_sorting(git2::Sort::TOPOLOGICAL)?;
    let collector_pb = ProgressBar::new_spinner().with_style(
        ProgressStyle::with_template("Collecting commits {spinner}")
            .unwrap()
            .tick_chars("▖▘▝▗"),
    );

    collector_pb.enable_steady_tick(time::Duration::from_millis(500));
    let oids: Vec<git2::Oid> = revwalk.try_collect::<Vec<git2::Oid>>()?;
    collector_pb.disable_steady_tick();
    collector_pb.is_finished();
    return Ok(oids);
}
