use cron::Schedule;
use std::{
    fmt::Debug,
    path::{Path, PathBuf},
    str::{CharIndices, FromStr},
};
use thiserror::Error;

struct RunFinder<'a> {
    iter: CharIndices<'a>,
}

impl<'a> Iterator for RunFinder<'a> {
    type Item = (usize, usize);

    fn next(&mut self) -> Option<Self::Item> {
        let mut last_idx: usize;

        let start = loop {
            let (pos, c) = self.iter.next()?;

            last_idx = pos;

            if c.is_whitespace() {
                break pos;
            }
        };

        let end = loop {
            let Some((pos, c)) = self.iter.next() else {
                return Some((start, last_idx + 1));
            };

            last_idx = pos;

            if !c.is_whitespace() {
                break pos;
            }
        };

        Some((start, end))
    }
}

fn find_whitespace_runs(str: &str) -> impl Iterator<Item = (usize, usize)> + use<'_> {
    RunFinder {
        iter: str.char_indices(),
    }
}

#[derive(Debug, Error)]
#[error("Invalid crontab line")]
pub struct InvalidFormatError {
    source: Option<anyhow::Error>,
}

pub struct CronJob {
    pub schedule: Schedule,
    pub command: String,
}

impl FromStr for CronJob {
    type Err = InvalidFormatError;

    fn from_str(line: &str) -> Result<Self, Self::Err> {
        // Split on runs of whitespace
        let mut splitter = find_whitespace_runs(line);

        let brk = if line.starts_with("@") {
            // Schedule is an @alias, split on first whitespace run.
            splitter.nth(0)
        } else {
            // Schedule is a six-field cron expr, split on sixth whitespace run.
            splitter.nth(5)
        };

        let (spec_end, command_start) = brk.ok_or_else(|| InvalidFormatError { source: None })?;
        let spec = &line[..spec_end];
        let command = &line[command_start..];

        let schedule = Schedule::from_str(spec).map_err(|source| InvalidFormatError {
            source: Some(anyhow::Error::from(source)),
        })?;

        Ok(CronJob {
            schedule,
            command: String::from(command),
        })
    }
}

#[derive(Debug, Error)]
pub enum CronTabError {
    #[error("Error reading from crontab at {path}")]
    IoError {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error(
        "Invalid crontab entry on line {line_no}. Cron expressions must \
            consist of six(!) space-separated fields or an alias that \
            starts with @. Environment variable specifications are not \
            supported."
    )]
    InvalidFormat {
        line_no: usize,
        source: InvalidFormatError,
    },
}

fn read_crontab(file: &str) -> Result<Vec<CronJob>, CronTabError> {
    let mut jobs: Vec<CronJob> = Vec::new();

    for (line_idx, line) in file.split("\n").enumerate() {
        let line = line.trim();

        if line.is_empty() || line.starts_with("#") {
            continue;
        }

        let job = CronJob::from_str(line).map_err(|source| CronTabError::InvalidFormat {
            line_no: line_idx + 1,
            source,
        })?;

        jobs.push(job);
    }

    Ok(jobs)
}

pub fn load_crontab(path: &Path) -> Result<Vec<CronJob>, CronTabError> {
    let file = std::fs::read_to_string(path).map_err(|source| CronTabError::IoError {
        path: path.to_path_buf(),
        source,
    })?;

    read_crontab(&file)
}

#[cfg(test)]
mod tests {
    use chrono::DateTime;

    use super::*;

    #[test]
    fn test_whitespace_runs() {
        let s = "  a bb   c   ";
        //       01234567890123
        let mut iter = find_whitespace_runs(s);

        let (start, end) = iter.next().unwrap();
        assert_eq!(start, 0);
        assert_eq!(end, 2);

        let (start, end) = iter.next().unwrap();
        assert_eq!(start, 3);
        assert_eq!(end, 4);

        let (start, end) = iter.next().unwrap();
        assert_eq!(start, 6);
        assert_eq!(end, 9);

        let (start, end) = iter.next().unwrap();
        assert_eq!(start, 10);
        assert_eq!(end, 13);

        let None = iter.next() else { panic!() };
    }

    #[test]
    fn test_from_str() -> Result<(), anyhow::Error> {
        let job = CronJob::from_str("2   *   * * * * foo")?;
        let t0 = DateTime::parse_from_rfc3339("2000-01-01T00:00:10+00:00")?;
        let t1 = job.schedule.after(&t0).next().unwrap();

        assert_eq!(t1.to_rfc3339(), "2000-01-01T00:01:02+00:00");
        assert_eq!(job.command, "foo");

        // The 5th was a Wednesday, midnight on the 9th is the next Sunday.

        let job = CronJob::from_str("@weekly     bar")?;
        let t0 = DateTime::parse_from_rfc3339("2000-01-05T00:00:10+00:00")?;
        let t1 = job.schedule.after(&t0).next().unwrap();

        assert_eq!(t1.to_rfc3339(), "2000-01-09T00:00:00+00:00");
        assert_eq!(job.command, "bar");

        Ok(())
    }

    #[test]
    fn test_read_crontab() -> Result<(), anyhow::Error> {
        // Example shamelessly stolen from the crontab(5) man page.
        // Fixed up to six-field format, added whitespace runs,
        // and added some @alias tests as well.

        let jobs = read_crontab(concat!(
            "       0  5 0 * * *       example_daily   \n",
            "   # run at 2:15pm on the first of every month\n",
            "\n",
            "       0 15  14 1 * *     example_monthly\n",
            "\n",
            "   # run at 10 pm on weekdays\n",
            "       0 0 22  * * 1-5    example_weekdays\n",
            "\n",
            "   # run 23 minutes after midn, 2am, 4am ..., everyday\n",
            "       0 23 0-23/2 *  * * example_every_other_hour\n",
            "\n",
            "   # run at 5 after 4 every sunday\n",
            "       0 5 4 * *  sun     example_sunday\n",
            "\n",
            "   # test alias\n",
            "   @monthly example_alias\n"
        ))?;

        assert_eq!(jobs.len(), 6);

        assert_eq!(jobs[0].schedule.to_string(), "0  5 0 * * *");
        assert_eq!(jobs[0].command, "example_daily");

        assert_eq!(jobs[1].schedule.to_string(), "0 15  14 1 * *");
        assert_eq!(jobs[1].command, "example_monthly");

        assert_eq!(jobs[2].schedule.to_string(), "0 0 22  * * 1-5");
        assert_eq!(jobs[2].command, "example_weekdays");

        assert_eq!(jobs[3].schedule.to_string(), "0 23 0-23/2 *  * *");
        assert_eq!(jobs[3].command, "example_every_other_hour");

        assert_eq!(jobs[4].schedule.to_string(), "0 5 4 * *  sun");
        assert_eq!(jobs[4].command, "example_sunday");

        assert_eq!(jobs[5].schedule.to_string(), "@monthly");
        assert_eq!(jobs[5].command, "example_alias");

        Ok(())
    }
}
