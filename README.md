# Pursue

A personal project and attempt at writing [pure](https://github.com/sindresorhus/pure) entirely in [Rust](https://www.rust-lang.org/). Also heavily inspired by [purs](https://github.com/xcambar/purs).

## Overview

A just as pretty and hopefully faster version of my favorite prompt.

### Why
- I wanted to learn Rust and was inspired by purs to use this as a starting project.
- I want a majority of my command line to be running Rust
- I think it's a great way to learn some asynchronous/threading to match some of the features in Pure

## Roadmap

Currently, most of the pre-command has been written synchronously, but I thought I'd lay out a timeline as goals for myself.

### Versions
- v0.1: Have both the pre-command and main line functional and able to be dropped into a current usage of pure (not easily).
- v0.2: Match most of the options and flags of pure
- v0.3: Mutlithread most of the operations to improve speed
- v0.4: Fetch repo's in the background (?)
- v1: Make it easy to use as a zsh plugin (maybe even for other shells as well)
