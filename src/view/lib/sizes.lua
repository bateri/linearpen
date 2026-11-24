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
	-- The head's width (the list's: the page's, or less what stands beside the list) from which it
	-- holds Refresh, from which its filter is the wider one, and below which the filter has a row of its
	-- own: the room the longest name of a list has in its button (a name is cut at 26 letters) and the
	-- filter beside it, so that the head is one row from the room it has and not from what is in it.
	-- Where the room narrows the filter gives way first (260 to 200), then Refresh, then New issue.
	wide = 820,
	filter_room = 880,
	-- The list's room from which the button has the whole of a list's name (26 letters), and from which
	-- the head is one row at all (the name cut at 13 letters below `named`).
	named = 540,
	stacked = 440,
	-- The filter's width: it is the same whatever it holds, and a little more where the head is wide.
	filter = 200,
	filter_wide = 260,
	-- The page's width from which the bar of the selection has all the values, and from which four.
	bar_all = 1000,
	bar_four = 640,
	-- The width of the label column of an issue, and of the date and the name of whose it is.
	identifier = 76,
	date = 52,
	who = 56,
	-- A row of the inbox: its note (who, and what it is about), where the list is this wide, whatever
	-- stands beside it (it says why the row is there); in a narrower one what it is about alone.
	note = 150,
	note_short = 96,
	noted = 760,
	-- A row of the list of projects: the word of its health, its target, and how far it is (the ring
	-- and `100%`).
	health = 84,
	target = 80,
	progress = 48,
	-- A page this wide shows the labels of an issue, and this much the name of whose it is.
	labels = 760,
	assignee = 880,
	-- The head's width from which it holds New issue (below it the key is in the line of keys).
	roomy = 760,
	-- What stands beside the list: at least this wide (the three buttons over an issue are whole), and,
	-- until the person sizes it, a part of the page up to the most. The person sizes it by its edge or
	-- with the keys, a step at a time, up to what leaves the list the room of the fullest head it can
	-- have (`wide`, else `roomy`, else `stacked`: `panel.most`).
	side_least = 420,
	side_most = 520,
	side_share = 0.44,
	side_step = 40,
	-- The edge between the list and what stands beside it, the kit's split's (its `GRIP`), and how far
	-- its arrow keys move it while it has the keyboard (the kit's `STEP`).
	edge = 7,
	edge_step = 16,
	-- The tabs of a wide head (My issues, All issues, Inbox, Projects, and the button of every other
	-- list): what they take more than the list's button does, which the head's widths grow by where it
	-- has them. The page's width alone says whether it has them (`tabs_from`, below), so that an issue
	-- opening does not change the head.
	tabs_extra = 300,
	-- The most of another list's name the button after the tabs shows, and where the list has less room
	-- (`named` and the tabs' more), less of it and a narrower filter.
	tab_name = 16,
	tab_name_short = 8,
	tab_filter = 160,
	-- The least room of a list whose head has the tabs: the four, another list's name cut short, the
	-- narrower filter.
	tabbed_least = 720,
	-- Of the 80 columns of a pane: a page this short drops what it can.
	short = 440,
}

-- The page's width from which the issue (or a form) stands beside the list: the least of it, the edge
-- and a list whose head is one row (its name cut short). Below it, it has the pane (a head of two rows would move the list
-- as the issue opens, and the edge could not move).
SIZE.beside = SIZE.side_least + SIZE.edge + SIZE.stacked

-- The page's width from which the head has the tabs: the issue at its least beside a list whose head has
-- them and the filter on one row, at their narrowest.
SIZE.tabs_from = SIZE.side_least + SIZE.edge + SIZE.tabbed_least

return SIZE
