# Phoenix

Phoenix is a chunk-based file synchronization platform using a custom binary
protocol that sits on-top of the [Noise Protocol](https://noiseprotocol.org/).

## The State of File Synchronization

Current file synchronization tools tend to fit into two buckets:

1. Manual/on-demand synchronization
2. Centralized event based file synchronization

The first group is very well developed and absolutely amazing tools exist for
it. `rsync(1)` is a classic example of this.

The second bucket as a lot of very common tools in it, but none of them are
really good. These would be services like Google Drive, Dropbox, OneDrive, and
even self-hosted tools like Nextcloud. The current solutions in the field are
poorly built and seem to think of file transfers as an afterthought to
pre-existing web standards. 

## Phoenix to the Rescue!

Phoenix focuses on the second bucket listed above: centralized event based file
synchronization. 

To solve the issues with current tooling,the solution as to be approached with
the number one priority being securely and robustly sending files. For this
concept to work, an entire new ecosystem will have to be created; client,
protocol, and server included.
