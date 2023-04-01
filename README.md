# PWD interface

This is a simple and **insecure** HTTP server that can serve a folder of files
and receive new files. It's an attempt to ease the use of
[play-with-docker](https://labs.play-with-docker.com/) instances when pushing
and pulling files into/from them.

## Usage

In the PWD instance:

```bash
docker run --rm --name pwdi -d -p 80:80 -v $(pwd):/srv plotter/pwd-interface
docker logs pwdi
```

Take note of the token shown in the logs, and then **expose port 80** via the
web interface and copy the new tab's URL. These two values (the token and URL)
must be given to the client.

In your local machine (grab the client from the [releases
page](https://github.com/kklingenberg/pwd-interface/releases)):

```bash
pwdi-client
```

Then use `token <token>` to configure the token and `server <url>` to configure
the URL you got from the PWD instance. Finally, use `pull` and `push` to fetch
files from, and send files to the PWD instance.

You may also add a `.pwdiignore` file at the root of your local folder to
exclude some files from the transfer ([syntax
reference](https://git-scm.com/docs/gitignore#_pattern_format)).
