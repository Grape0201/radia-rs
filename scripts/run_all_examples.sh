#!/bin/bash

for f in examples/*.yaml; do
    ./target/release/radia-cli $f
done
