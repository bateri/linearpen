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
	-- The sizes of words.
	body = 14,
	small = 13,
	tiny = 12,
	-- The list's room (not the page's) from which the head holds Refresh, and below which the filter
	-- has a row of its own.
	wide = 880,
	narrow = 640,
	-- The width of the label column of an issue, and of the date and the name of whose it is.
	identifier = 62,
	date = 52,
	who = 56,
	-- A page this wide shows the labels of an issue, and this much the name of whose it is.
	labels = 760,
	assignee = 880,
	-- The list's room from which the head holds New issue (below it the key is in the line of keys).
	roomy = 760,
	-- Of the 80 columns of a pane: a page this short drops what it can.
	short = 440,
}

return SIZE
