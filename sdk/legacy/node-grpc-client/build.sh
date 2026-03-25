#!/bin/bash

protoc --plugin=$(npm root)/.bin/protoc-gen-ts_proto \
    --ts_proto_out=src \
    --ts_proto_opt=outputServices=grpc-js \
    --ts_proto_opt=esModuleInterop=true \
    -I=../../crates/types-traits/grpc-api-types/proto \
    ../../crates/types-traits/grpc-api-types/proto/**/*.proto
