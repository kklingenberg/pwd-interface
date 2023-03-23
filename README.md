# PWD interface

This is a simple and insecure HTTP server that can serve a folder of files and
receive new files, which will be placed as randomly named in the same
folder. It's an attempt to ease the use of
[play-with-docker](https://labs.play-with-docker.com/) instances when pushing
and pulling files into/from them.

## Usage

```bash
docker run --rm -p 80:80 -v /path/to/folder:/srv plotter/pwd-interface
```

To push a file, use `curl` with a multipart upload, alongside the `--user`
option shown in the console. The randomly generated name is returned in the
response.

To pull a file, also use given credentials and access the file by name.

_If_ you're using this by some chance and your data is sensitive, I'd recommend
you stop the container as soon as you're done with the file exchange.
