-- Sizes in points, from the design (mockup/variants/linearpen.js): a row of the list, the head of a
-- group, the head of the page and the line of keys under it.

local SIZE = {
	-- A row of the list, and the head of a group (with the four points above it).
	row = 34,
	group = 34,
	group_gap = 4,
	-- The room around the rows and the head's own padding.
	pad = 16,
	head_top = 13,
	head_bottom = 9,
	-- The height of a field and of a button, and of a smaller one.
	control = 28,
	small_control = 24,
	-- A button of the bar of the selection: the bar is as high as the line of keys, and a button
	-- stands in it.
	bar_control = 18,
	-- The sizes of words.
	body = 14,
	small = 13,
	tiny = 12,
	-- The page's width (the head is the page's, whatever stands beside the list) from which the head
	-- holds Refresh, and below which the filter has a row of its own: the room the longest name of a
	-- list has in its button (a name is cut at 26 letters) and the filter beside it, so that the head is
	-- one row from the width of the pane and not from what is in it.
	wide = 880,
	stacked = 540,
	-- The filter's width: it is the same whatever it holds, and a little more where the page is wide.
	filter = 200,
	filter_wide = 260,
	-- The page's width from which the bar of the selection has all the values, and from which four.
	bar_all = 1000,
	bar_four = 640,
	-- The width of the label column of an issue, and of the date and the name of whose it is.
	identifier = 76,
	date = 52,
	who = 56,
	-- A page this wide shows the labels of an issue, and this much the name of whose it is.
	labels = 760,
	assignee = 880,
	-- The page's width from which the head holds New issue (below it the key is in the line of keys).
	roomy = 760,
	-- Of the 80 columns of a pane: a page this short drops what it can.
	short = 440,
}

return SIZE
