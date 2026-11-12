# Repository initialisation and adoption

## `repowill init [PATH]`

Create a new repository with RepoWill's scaffold and initial commit.

```sh
repowill init /path/to/repo --name "My Project" --email "me@example.com"
```

### Flags

- `--name <NAME>` — project owner name (required in non-interactive mode)
- `--email <EMAIL>` — project owner email (required in non-interactive mode)
- `--dry-run` — show the file plan without writing anything
- `--answers <FILE>` - read all options from a TOML answers file
- `--resume` - resume an interrupted init (skips already-written files)
- `--non-interactive` - fail instead of prompting when identity is missing

### Interactive wizard

Running `repowill init` without `--name`/`--email`/`--answers` launches an interactive wizard
that walks through folder selection, project details, preset choice, and confirmation.

### Answers file

```toml
path = "C:/Projects/my-app"
mode = "new"                    # new | adopt
name = "my-app"
description = ""
preset = "recommended"          # starter|recommended|strict|maximum
stacks = ["auto"]
identity = { name = "AfterLeads", email = "afterleadseg@gmail.com" }
project_license = "MIT"
files = { gitattributes = true, gitignore = true, editorconfig = true, readme = true, license = true, changelog = true }
```

## `repowill adopt [PATH]`

Adopt an existing repository: detect stacks, add missing scaffold files (never overwriting),
configure identity. Requires a git repository.

```sh
repowill adopt . --name "My Project" --email "me@example.com"
repowill adopt . --dry-run   # preview only
```

## `repowill detect [PATH]`

Print detected stacks, hook managers, agent configs, and existing files as JSON.

## `repowill update [PATH]`

Re-apply scaffold files that are missing (never overwrites user edits). Creates a commit
only if files were added.

## `repowill projects list|add|forget`

Manage the global projects registry.
