#!/bin/sh

if [ -d "./dist" ]; then
    echo "'./dist' already exists";
    exit -1;
fi

cargo $@

project_dir="$(dirname $0)"

mkdir dist
cp -v "$project_dir/target/release/rust_i_telex_teilnehmerserver" dist
cp -v -r "$project_dir/templates" dist
cp -v -r "$project_dir/static" dist
cp -v "$project_dir/Rocket.toml" dist
cp -v "$project_dir/template.env" dist
cp -v "$project_dir/localisation.json" dist