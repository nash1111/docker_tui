#!/bin/bash

NUM_CONTAINERS=50

for i in $(seq 1 $NUM_CONTAINERS); do
    docker run -d --name test_container_$i alpine sleep 3600
done
