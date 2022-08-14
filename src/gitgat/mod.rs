extern crate git2;

use git2::{DiffDelta, DiffLine, DiffOptions, Oid, Repository, Sort};
use indicatif::{ProgressBar, ProgressIterator, ProgressStyle};
use std::time::Duration;

/// Options for running gitgat.
pub struct Opts<'a> {
    pub author: &'a str,
    pub excluded_dirs: Vec<&'a str>,
}

/// Run gitgat on a repository.
pub fn run(opts: Opts) {
    let repo = match Repository::open("/home/johan/core3/src") {
        Ok(repo) => repo,
        Err(e) => panic!("failed to open: {}", e),
    };

    let oids = collect_oids(&repo);

    let mut commits = 0;
    let mut insertions: u32 = 0;
    let mut deletions: u32 = 0;

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

        commits += 1;
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
                // println!("{}", String::from_utf8(hunk.header().to_vec()).unwrap());
                if line.origin() == '+' {
                    insertions += 1;
                } else if line.origin() == '-' {
                    deletions += 1;
                };
                return true;
            }),
        )
        .unwrap();
    }
    println!(" {} commits", commits);
    println!("+{}", insertions);
    println!("-{}", deletions);
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

fn oid_progress_style() -> ProgressStyle {
    ProgressStyle::with_template(
        "[{elapsed_precise}] [{bar:30.green}] {human_pos:>7}/{human_len:7} commits",
    )
    .unwrap()
    .progress_chars("▮ ")
}
