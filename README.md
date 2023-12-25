# Gpt for UDS

This project is a linux daemon mean to run in the background through Systemd for making it easier integrating other applications with ChatGPT.

## Contributing
I'm new to rust and new to making linux deb packages. I'm also new to working with linux domain socket protocols.
If you want to contribute, please reach out or send me a PR.

## Supported systems
All debian based distros should be supported. Currently only x64 platforms.

## How to use
Install the latest .deb package found in releases on github and install it with `sudo apt install ./gpt-for-uds*.deb`
Open up the configuration file in `/etc/gpt-for-uds/gpt-for-uds.conf` and enter your OpenAI token. Any usage through this application will be billed from your account.

Restart the service using `sudo systemctl restart gpt-for-uds` to update the configuration and check that it is working correctly through `sudo systemctl status gpt-for-uds`.

The application should create some socket files under the `/run/gpt-for-uds/` directory. One for each supported ChatGPT model. These are owned by the user and group `gptforuds`.

Make sure that any user who is expected to use these sockets have the group `gptforuds`.

## How it works
The application creates sockets (UDS) that other applications can connect with. I chose a very simple protocol for this interface.

The client creates a String of a json for the list of messages for ChatGPT to respond to. For example:
```json
[{"actor":"System","message":"You are a code expert, answer all the users questions to the best of your ability. Try to find bugs in your own statements while going through it."},{"actor":"User","message":"Show me hello world in C#"}]
```

Count the number of bytes in this json string and send that to the socket as a u32 big endian number and send the json after.

Now go over to reading mode and in a loop until the connection is closed by the server do the following:

Read the first 4 bytes as a u32 big endian number representing the length of the next chunk of text.
Read this number of bytes from the connection.

To ask a follow up question send in the full history with the next question and repeat this again.
