#!/usr/bin/env bash

graphql-client introspect-schema https://gitlab.com/api/graphql > src/providers/graphql/gitlab/schema.json
wget https://docs.github.com/public/schema.docs.graphql -O src/providers/graphql/github/schema.graphql
