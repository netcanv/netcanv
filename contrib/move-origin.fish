#!/usr/bin/env fish

# move-origin.fish
# --------------
# Usage: move-origin.fish <path> <x> <y>
# Moves a .netcanv canvas's origin to the provided coordinates.

if [ (count $argv) -ne 3 ]
  echo "usage: move-origin.fish <path> <x> <y>"
  exit 1
end

set path $argv[1]
set origin_x $argv[2]
set origin_y $argv[3]

# Create the work directory.
set work_dir "$path/move-canvas-workdir"
mkdir "$work_dir" || exit 1

function move-png -a png
  # Parse the position.
  set -l raw_coordinates (basename -- $png .png)
  set -l original_position (string split , -- $raw_coordinates)
  set -l x $original_position[1]
  set -l y $original_position[2]
  # Offset the coordinates by the origin.
  set -l new_x (math -- "$x" - "$origin_x")
  set -l new_y (math -- "$y" - "$origin_y")
  mv -- $png "$work_dir/$new_x,$new_y.png"
end

# Iterate over all PNG files in the folder, and start their move jobs.
for png in "$path"/*.png
  move-png $png &
end

# Wait for everything to complete, and move the files out of the work directory
# into the original path.
wait
mv $work_dir/*.png $path

# Clean up.
rmdir "$work_dir"

