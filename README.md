> [!TIP]
If you like this, check out [atem](https://github.com/jullanggit/atem)!

## About

Rebos is a tool that aims at mimicking what NixOS does (repeatability), for any Linux distribution.

It achieves this by using a generation system and calculating diffs between generations,
thus, it can also remove old entries (packages, services, groups, etc...) from the system.

Rebos works on any Linux distro. In order to remove packages and services and stuff,
you create managers, and Rebos executes the commands specified with the managers
and the arguments interpolated in.

Rebos also has a lot of features for ease of use.

Everything is covered very well in the wiki.



## Why?

Rebos was created to solve a huge issue that NixOS solves in a very over-complicated way.
That issue is, if you have to reinstall Linux, is there an elegant way to simply set everything
up again the way it was with just a few simple commands? Is there an elegant way to install all packages,
enable all services, add all groups to the user, etc..? Well, NixOS does solve this issue but
using NixOS can be very complicated and frustrating, and the lack of FHS-compliancy can be very annoying.
Sometimes you just want to use that one epic distro, but you still want repeatability. Many
people settle on using a script, but there are some major issues with that. Those issues being that scripts
are only for a single time so syncing changes across multiple machines with each other is out of the question,
and scripts not having an elegant way to remove packages and services etc... This is why something like Rebos is a great solution.
It is simple enough to be a standalone application that works on any distro, but complex enough to be an elegant solution!



## Installation

You can install Rebos from crates.io:
```bash
cargo install rebos
```

If you are on Arch Linux, you can get it from the [oglo-arch-repo](https://gitlab.com/Oglo12/oglo-arch-repo).



## Closing Words

Rebos is not just a random half-assed project. This is something me and many others use every day.
So if you see an issue, please report it in order to improve the tool! I have more than one main machine,
so Rebos is 100% required for my day-to-day workflow. I hope that is proof to this being an essential tool
that will never just be abandoned.

Created by OgloTheNerd, with lots of ❤️ and 🕒 put into it. I hope you enjoy!



#### [Click to visit the wiki.](https://gitlab.com/Oglo12/rebos/-/wikis/home)
