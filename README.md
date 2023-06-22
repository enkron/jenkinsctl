# jenkinsctl
Jenkins manager tool

The project aims to wrap Jenkins json api and some post requests which
can be performed on Jenkins parts (such as 'delete node').

It is a very early stage of development (it'd be better to say hobby project to learn Rust).
Current functionality includes:
- shutdown  Set 'prepare to shutdown' bunner with optional reason
	- on    Set shutdown banner
	- off   Cancel shutdown
- restart   Restart Jenkins instance (soft/hard)
- copy      Copy job from the existing one
	- job   Copy job
	- view  Copy view
- node      Node actions
	- show  Show node information
	- list  List all (with optional status information)
- job
    - list  Recursively list all the jobs in an instance
