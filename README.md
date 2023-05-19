# (WIP) Stackable

This repository contains the Stackable library, the Stackable CLI tool, and the Stackable server.

## Components

- `stackable-lib`: The underlying library for all actions related to the Stackable Data Platform
- `stackablectl`: CLI tool to interact with local and remote deployments of the data platform. [Link][ctl-readme]
- `stackabled`: API server used by frontends to interact with the data platform. [Link][daemon-readme]

[daemon-readme]: ./bins/stackabled/README.md
[ctl-readme]: ./bins/stackablectl/README.md

## Developer Setup

This repository ships custom Git hooks. They are located in the `.githooks` directory. Before you enable them, make sure
to check out the code they execute. Currently, the `pre-commit` hook runs two tasks: `gen-man` and `gen-comp`. See
[here][xtasks] what these tasks do. To enable all hooks inside the directory, use the following command:

```shell
git config --local core.hooksPath .githooks/
```

[xtasks]: ./xtask/src/main.rs
