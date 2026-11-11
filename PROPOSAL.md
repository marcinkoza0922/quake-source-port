Write a full fledged Quake source port from scratch. It should be written in Rust using a Vulkan renderer and custom, fit-to-purpose ECS engine.

The repos to ironwail and vkQuake are included as well as a reference, as is a symlink to a directory where I have a bunch of quake maps installed. Exclude these from the git repo. 

It should be compatible with everything ironwail or vkQuake are compatible with, with special attention paid to Arcane Dimensions. It should also be compatible with the two official Machine Game wads included (under mg1 and mg3)

Third party dependencies should be light and be only used for "solved" problems, primitive interactions with the underlying system, or things that are trivial. Anything that is necessarily opinionated, like the engine or gui, needs to be built custom-fit for the project.

The game should have controller support with industry standard aim-assist practices. It should also have a nicer mod-menu.

Performance is key. It should at worst be within spitting distance of ironwail or vkQuake, and ideally even better than those.

Build a testing framework to automatically traverse each level in a way that finds each secret and kills every enemy on the hardest difficulty, though god mode can be enabled during this.

Give discrete tasks that do not interfere with each other to subagents.
