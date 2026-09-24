#!/usr/bin/env nu
# Builds rustdoc (including private items) into <trunk staging dir>/docs.
# Run by Trunk as a post_build hook; see Trunk.toml.

def main [] {
  cd ($env.TRUNK_SOURCE_DIR? | default ".")

  let target = "wasm32-unknown-unknown"
  let out = ($env.TRUNK_STAGING_DIR? | default "dist") | path join "docs"

  ^cargo doc --document-private-items --no-deps --target $target

  let target_dir = (^cargo metadata --format-version 1 --no-deps | from json).target_directory
  let doc_dir = $target_dir | path join $target "doc"

  rm -rf $out
  mkdir $out
  ls $doc_dir | each {|entry| cp -r $entry.name $out }

  # rustdoc has no root index for a single crate; redirect docs/ to it.
  [
    '<!DOCTYPE html>'
    '<meta charset="utf-8">'
    '<meta http-equiv="refresh" content="0; url=wgpu_test/index.html">'
    '<a href="wgpu_test/index.html">Documentation</a>'
  ] | str join "\n" | save -f ($out | path join "index.html")
}
