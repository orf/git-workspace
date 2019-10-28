# git-workspace
[![Crates.io](https://img.shields.io/crates/v/git-workspace.svg)](https://crates.io/crates/ripgrep)
[![Actions Status](https://github.com/orf/git-workspace/workflows/CI/badge.svg)](https://github.com/orf/git-workspace/actions)

![](./images/demo.gif)

If your company has a large number of repositories and your work involves jumping between then, then `git-workspace` can save you some time by:

* Easily synchronizing your projects directory with **Github**, **Gitlab.com** or **Gitlab self-hosted** :wrench:
* Keep projects consistently named and under the correct path :file_folder:
* Automatically set upstreams for forks :zap:
* Move deleted repositories to an archive directory :floppy_disk:
* Allowing you to access any repository instantly :shipit:
* Execute `git fetch` on all projects in parallel :godmode:

This may sound useless, but the "log into your git provider, browse to the project, copy the clone URL, devise a suitable path to clone it" dance can be a big slowdown. The only obvious solution here is to spend more time than you'll ever spend doing this in your whole life on writing a tool in Rust to do it for you.

# Install

## Homebrew (MacOS + Linux)

`brew tap orf/brew && brew install git-workspace`

## Binaries (Windows)

Download the latest release from [the github releases page](https://github.com/orf/git-workspace/releases). Extract it 
and move it to a directory on your `PATH`.

## Cargo

`cargo install git-workspace`
