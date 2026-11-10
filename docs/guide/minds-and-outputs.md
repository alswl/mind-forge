# Create a mind and its outputs

Start in an initialized Mind Repo. Use `mf init` to initialize an empty directory.
A Mind groups shared intent, reasoning, sources, and assets. It can have zero or
more editable Outputs. Each Output can use one file or ordered Markdown blocks.

1. Create a project with an explicit layout:

   ```sh
   mf project new research --layout category
   ```

   Use `--layout bundle` to group each Mind's content in its own directory.
   Layouts can coexist in one repository. Creation does not convert existing content.

2. Create a Mind:

   ```sh
   mf mind new brief -p research
   mf mind show projects/research::brief
   ```

   A Mind with no Outputs is valid. Missing optional roles are not created as fake paths.

3. Create the selected deliverables:

   ```sh
   mf output new brief design -p research
   mf output new brief summary --file -p research
   mf output list projects/research::brief
   ```

   The default Output shape is a directory of blocks. `--file` selects one Markdown file.
   Creation rejects existing names and case or file/directory collisions.

4. Build the selected Output:

   ```sh
   mf build projects/research::brief::design
   mf build projects/research::brief::design --format html
   ```

   The products are `build/brief/design.md` and `build/brief/design.html`.
   The editable source remains separate from these products.

Use identities returned by list/show commands. Do not derive them from display titles.
A full identity selects its project. A conflicting `-p` is an error.
A Mind with one Output can select it implicitly. Multiple Outputs need an explicit
selection or a valid `default_output` in the Mind registration. Building a Mind
with zero Outputs fails.

For exact paths, fields, discovery rules, and compatibility, use the
[content layout reference](../manual/content-layout.md).
To rename or move content, use [lifecycle commands](manage-content.md).
To write with an agent, use the [Skills guide](../../skills/README.md).
