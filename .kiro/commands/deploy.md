# Deploy to Fly.io

Deploy the Murdoch bot to Fly.io:

## Prerequisites

- `flyctl` CLI installed
- Logged in via `flyctl auth login`
- App "murdoch-bot" exists

## Build and Deploy

```bash
flyctl deploy --remote-only
```

## Check Logs

```bash
flyctl logs -a murdoch-bot
```

## Check Status

```bash
flyctl status -a murdoch-bot
```

## SSH into Machine (if needed)

```bash
flyctl ssh console -a murdoch-bot
```

## Database Location

SQLite database is stored at `/data/murdoch.db` on the persistent volume.
