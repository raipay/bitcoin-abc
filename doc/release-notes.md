# Bitcoin ABC 0.28.4 Release Notes

Bitcoin ABC version 0.28.4 is now available from:

  <https://download.bitcoinabc.org/0.28.4/>

This release includes the following features and fixes:
 - The avalanche peers are now dumped to a file upon node shutdown and reloaded
   upon startup to allow for faster avalanche bootstraping. This feature can be
   disabled by using the `-persistavapeers=0` option.
 - `bitcoin-seeder` was not working properly since version 0.28.2 and has been
   fixed. While crawling for peers appropriately, it would have fail to mark any
   peer as good due to a bug.

Network upgrade
---------------

At the MTP time of `1715774400` (May 15, 2024 12:00:00 UTC), the following
changes will become activated:
 - Bump automatic replay protection to the next upgrade, timestamp `1731672000`
   (November 15, 2024 12:00:00 UTC).
 - Add a new checkpoint after the May 15th, 2024 eCash network upgrade
