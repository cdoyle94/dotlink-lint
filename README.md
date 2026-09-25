# dotlink-lint

A linter for dotfiles symlink manifests.

## the problem

Most dotfiles setups boil down to a list of "put this repo file at that
home directory path" mappings, applied with `ln -s`. Whether that list
lives in a shell script, a Makefile, or GNU Stow's directory layout, it's
easy for it to quietly rot:

- two entries point at the same target, so the second `ln -s` silently
  overwrites the first with no warning
- a copy-pasted line still points at the old target after a rename
- a path typo means the symlink never gets created and you don't notice
  until the day you actually needed that config
- a source path with a stray `../` reaches outside the dotfiles repo

None of these show up as a shell error. They show up as "why is my prompt
theme different on this machine" three weeks later.

`dotlink-lint` reads a plain-text manifest of intended symlinks and reports
problems with exact line numbers, the way a compiler would, instead of
letting them surface at `ln -s` time.

## manifest format

One mapping per line, `source -> target`. Blank lines and lines starting
with `#` are ignored.

```
# personal dotfiles
zsh/zshrc      -> ~/.zshrc
zsh/zprofile   -> ~/.zsh_profile
git/gitconfig  -> ~/.gitconfig
vim/vimrc      -> ~/.vimrc
```

`source` is a path relative to the root of your dotfiles repo, and must
exist on disk there. `target` should start with `~/` or `/` so it resolves
the same way no matter where you run the tool from.

The repo root is currently just the directory the manifest file lives in;
there's no separate config for it yet.

## usage

```
$ dotlink-lint manifest.txt
manifest.txt:6: error: target '~/.vimrc' is already claimed at line 4 and would be overwritten
manifest.txt:9: warning: target 'config/nvim' should start with '~/' or '/' so it resolves the same way regardless of cwd
```

The process exits `1` if any errors were reported, `0` otherwise. Warnings
alone don't fail the run.

## build

Standard library only, no dependencies to fetch:

```
cargo build --release
./target/release/dotlink-lint manifest.txt
```

## status

Early skeleton. Checks so far: duplicate sources, duplicate targets, empty
fields, sources that escape the repo via `..`, sources that don't exist on
disk, and targets that aren't rooted under `~` or `/`. See the tests in
`src/linter.rs` for the exact cases covered.
