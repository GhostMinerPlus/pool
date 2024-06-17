# Edge

## What is Edge?
A data engine.

## Quick start
```sh
pool [config.toml] --port 8005
```
config.toml
```toml
# name = "pool"
# ip = "0.0.0.0"
# port = 80
db_url = "mysql://user:pass@host/database"
# thread_num = 8
# log_level = "INFO"
```
Then it will serve at http://$ip:$port/$name

## Usage
curl http://$ip:$port/$name/execute -X POST --data "_ return any"

## Script

## Atomic code
- set: clear all target then insert a target to "source->>code"
- insert: insert a new target to "source->>code"
- return: end the script and return a value
- dump: end the script then dump a value and return the string
- delete: delete a pool by id
- dc
- dc_ns
- dc_nt
