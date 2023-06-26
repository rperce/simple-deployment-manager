# SImple DEployment Manager

`sidem` is a simple webserver that listens for requests and, upon receipt, runs a
configured shell script.

The impetus and only truly supported use case is to call it in a webhook and have the
shell script it runs be to update and restart an application, but you can do whatever
you want with it.

## Setup

Download the latest version of `sidem` from the releases tab and get it on your server.
If your server uses systemd, there's a convenient `.service` file supplied with the
release.

Create a TOML file somewhere readable by whatever use will be executing the `sidem`
application. The systemd service file defaults to `/etc/sidem/sidem.toml`. It can be
passed with the `--config` command-line flag or in the `SIDEM_CONFIG` environment
variable.

An example configuration file:

```toml
## Global configuration. Both are optional; values here are defaults.
# host = "0.0.0.0"
# port = 6391

## Optional: require authorization, via Basic or Bearer HTTP Authorization header.
## If both are specified, accepts either. Leaving keys unset disables authorization
## (the default). Setting one but not both of auth.basic.user and auth.basic.pass
## is an error.
# [auth.basic]
# user = ""
# pass = ""
#
# [auth.bearer]
# token = ""

[[deployment]]
## This is an example deployment configuration for sidem itself.
name = "sidem" # Required within deployment: trigger on requests to /deploy/:name

## Either `script` or `command` and `args` is required. Supplying `script` and either
## `command` or `args` is an error; `args` may be omitted from `command`.
##
## `script` is passed to `bash`. If your system does not have bash installed, or you
## want to use a different shell, use `command` and call that shell directly or make
## your script executable and set its shebang accordingly.
script = "/etc/sidem/deployments/sidem.bash"
# command = "ruby"
# args = ["-e", "puts :hello"]
```

## Usage

Send a POST request to `sidem` at `/deploy/:name`. This will trigger the script and
return immediately. To check on the run status, GET `/status/:name`. For example, with
the configuration

```toml
[[deployment]]
name = "example"
command = "sh"
args = ["-c", "sleep 1; echo success"]
```

you can run

```
$ curl -sXPOST localhost:6391/deploy/example | jq .
{
  "name": "example",
  "state": "Starting",
  "last_deployed": null,
  "stdout": null,
  "stderr": null,
  "exit_code": null
}
$ curl -s localhost:6391/status/example | jq .
{
  "name": "example",
  "state": "InProgress",
  "last_deployed": "2023-06-26T21:00:03.481907532Z",
  "stdout": null,
  "stderr": null,
  "exit_code": null
}
$ sleep 2; curl -s localhost:6391/status/example | jq .
{
  "name": "example",
  "state": "Completed",
  "last_deployed": "2023-06-26T21:00:03.481907532Z",
  "stdout": "success\n",
  "stderr": "",
  "exit_code": 0
}
```
