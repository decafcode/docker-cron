# docker-cron

A service that calls the Docker API to start existing Docker containers in accordance with a configuration file following the six-field crontab format, where the commands in this crontab are interpreted as container names. That's it.

This seems like the sort of thing that ought to already exist, but if it does then I wasn't able to find it. I also used this project as an excuse to learn Rust, so it probably doesn't follow established idioms or best practices very well.

## Usage

This project is distributed as a container image which can be pulled from `ghcr.io/decafcode/docker-cron`. Mount a suitable crontab at `/etc/crontab` inside the container and mount a Docker-compatible API socket at the standard path of `/var/run/docker.sock`.

All schedules are interpreted in UTC time. The ability to define schedules relative to other time zones is not currently supported.

The containers that run the scheduled jobs need to be created and configured ahead of time, and that task is outside the scope of this tool.

Note that the crontab syntax for specifying environment variables is not supported, since the Docker API does not provide any way to supply additional environment variables to a container at container start time.

## Logging

This project uses the [tracing](https://github.com/tokio-rs/tracing) framework to write logs to stdout as JSON lines. By default it will log at the `INFO` level, which prints some startup messages and then logs a warning whenever a job exits with a nonzero exit code. Log verbosity can be controlled using the `RUST_LOG` environment variable as described in the tracing framework's [EnvFilter](https://docs.rs/tracing-subscriber/0.3.19/tracing_subscriber/filter/struct.EnvFilter.html#directives) documentation. The exact format of this service's log messages is not guaranteed to remain stable between releases, but a best effort will be made to minimize unnecessary changes.

```json
{"timestamp":"2025-07-10T16:34:06.200475Z","level":"INFO","fields":{"message":"Connecting to Docker"},"target":"docker_cron"}
{"timestamp":"2025-07-10T16:34:06.248455Z","level":"INFO","fields":{"message":"Docker connection OK, starting scheduler"},"target":"docker_cron"}
{"timestamp":"2025-07-10T16:35:00.112826Z","level":"WARN","fields":{"message":"Job did not succeed","status_code":1},"target":"docker_cron","span":{"container":"failing_example","schedule":"0 * * * * *","name":"schedule_job"},"spans":[{"container":"failing_example","schedule":"0 * * * * *","name":"schedule_job"}]}
{"timestamp":"2025-07-10T16:36:00.106210Z","level":"WARN","fields":{"message":"Job did not succeed","status_code":1},"target":"docker_cron","span":{"container":"failing_example","schedule":"0 * * * * *","name":"schedule_job"},"spans":[{"container":"failing_example","schedule":"0 * * * * *","name":"schedule_job"}]}
```

## License

MIT
