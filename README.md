PoorMansScreen
==============

The goal of this project is to replicate the "persistent sessions"
feature of [screen](https://www.gnu.org/software/screen/)

This will never be a feature complete screen implementation.
If you are intending on using it, make sure its what you expect it to be.

Design
======

The program is split into two parts.

Server
------

```
pms label bash -i
```

The server part will double-fork (double Command?) itself to daemonise a process.
That process will then create a Unix socket, for connections to the Client,
and then create a PTY (`/tmp/label`) and execute the given child process `bash -i`.
The server will wire the unix socket and pty together, so that STDIN/STDOUT/STDERR
are sent over the unix socket/

When run in server mode, the initial executable
will automatically connect to the server via the client half.

Client
------

```
pms label
```

The server portion will connect to the Unix socket created by the server portion (`/tmp/label`).
It will wire that connection up to STDIN/STDOUT of the current terminal,
which allows inputs to be sent to the child, and outputs to be displayed.

Done correctly, the experience should be pretty transparent.

`ctrl-a` + `ctrl-c` will exit the client, leaving the server side still running to be reconnected to.
`ctrl-c` on its own will SIGINT the server, and all parts should exit.

Roadmap
=======

????

No clear roadmap, this already does more than I need it to.
I would like to wrap the client side in some kind of UI so
that its clear when its a screen vs a normal terminal.

Open to suggestions, but given that `screen`, `tmux`, et al. all exist,
I suggest most people should use those.