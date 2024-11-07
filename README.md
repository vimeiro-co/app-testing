This is a testing and development app for the Matrix Rust SDK. This is particularly useful as a test bed to understand the SDK before bringing it into a mobile app via Uniffi, which has a much longer dev debug cycle.

## Setup

Create a `config.yaml` file patterned after `config.yaml.example`. Then run Cargo:

```
$ cargo run
```

The app will log in as the configured user and output a variety of messages. If you initiate emoji session verification from Element, the app will respond and automatically accept and confirm verification.


### Matrix SDK Update

A copy of the Matrix Rust SDK has been subtree'd into this repo at `matrix-rust-sdk/`. In order to update it, you'll need to do some [subtree magic](https://www.atlassian.com/git/tutorials/git-subtree). The procedure is:

1. Set a git remote to the upstream Rust repo: `git remote add matrix-rust-sdk git@github.com:matrix-org/matrix-rust-sdk.git` but DO NOT RUN A FETCH.

2. Update with the subtree command from the root of the repo: `git subtree pull --prefix matrix-rust-sdk/ matrix-rust-sdk main --squash`.

3. This will put you into a git merge cycle -- if you have made changes in `matrix-rust-sdk/` then you'll need to resolve conflicts and complete the merge. After this, you will have an updated sdk in a merge commit on HEAD. This can be rolled back via git if necessary.


### Timeline Testing

If you specify a `timeline_test_room` room id in `config.yaml`, the app will construct a matrix-sdk-ui timeline object a retrieve a short history of messages. Every 5 seconds it will output its current timeline to the log.
