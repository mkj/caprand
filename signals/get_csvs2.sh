#!/usr/bin/env zsh

fn="$1"

function ex() {
	dir="$1"
	fn="$2"

	(echo t_$dir,drive_$dir; grep INFO $fn |grep -v Initial | cut -d ' ' -f 4,5 | sed 's/ /,/'  ) > "$fn.$dir"
}

ex down "$fn"


