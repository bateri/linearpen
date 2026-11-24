-- The line under the page. With issues selected it is the bar of the selection (how many, the values
-- to change, More and Clear), as high as the line of keys that it stands in the place of; else it is
-- the line of keys (the library's `u:status`): what the keyboard does here, each key said once. A
-- key that has a button on the page is on the button, so the keys of Refresh and New issue are here only
-- where the pane is too narrow for their buttons. The two at the end of the line open the palette and
-- the card of keys, which hold the rest.

local kit = require("../lib/kit")
local commands = require("./commands")
local change = require("./change")
local filters = require("./filters")
local head = require("./head")
local list = require("./list")
local new = require("./new")
local picker = require("./picker")
local pictures = require("./pictures")
local work = require("./work")

local b, u, S, SIZE, act = kit.b, kit.u, kit.S, kit.SIZE, kit.act

local status = {}

-- How wide each hint is laid out, points, by the element that is its words: the library guesses the
-- width of an element, drops what does not fit by its guesses, and a guess that is short lets the line
-- run off a narrow pane.
local WIDTHS = setmetatable({}, { __mode = "k" })

-- The keys whose caps hold a word, by how many letters it has (the others are one sign: an arrow, ⌘, ⇧
-- or a letter).
local NAMED = { enter = 5, space = 5, escape = 3, home = 4, ["end"] = 3, tab = 3 }

-- How wide the caps of `specs` and the words `words` are laid out, as measured on the line drawn: a
-- cap of one sign is 16 points and a cap of a word 10 and 6 for each letter, 5 between the parts and
-- 8 on each side; a letter of the words is the library's own 6.6 (it adds 24 for the whole line). In
-- the cells of a terminal nothing was measured, and the library's own guess stands (nil).
local function measure(specs, words)
	if b.draws == nil or b.draws().cells == true then
		return nil
	end
	local width = 16
	for _, spec in specs do
		for part in string.gmatch(spec, "[^+]+") do
			local count = NAMED[part] or 1
			width += (count == 1 and 16 or 10 + 6 * count) + 5
		end
	end
	return width + utf8.len(words) * 6.6
end

-- The caps of the keys `specs`, as the library draws them, and the words `words` of what they do.
local function keyed(specs, words)
	local row = b.div():row():items_center():gap(5):no_shrink()
	for _, spec in specs do
		row:child(u:kbd(spec))
	end
	row:child(b.text(words))
	WIDTHS[row] = measure(specs, words)
	return row
end

-- The keys of a hint, as the library draws them, and its words. `ids` are commands, `words` what
-- they are said to do.
local function hint(ids, words)
	local specs = {}
	for _, id in ids do
		local first = commands.first(id)
		if first ~= nil then
			specs[#specs + 1] = first
		end
	end
	return keyed(specs, words)
end

-- The line of keys, of the items `parts` (`start`, `finish`), each as wide as it is laid out.
local function line(parts)
	for _, group in { parts.start or {}, parts.finish or {} } do
		for _, item in group do
			if item.width == nil and type(item.words) ~= "string" then
				item.width = WIDTHS[item.words]
			end
		end
	end
	return u:status("status", parts)
end

-- The two keys at the end of the line: the palette and the card of keys.
function status.finish()
	local finish = {}
	if S.width >= 620 then
		finish[#finish + 1] = {
			key = "palette",
			words = hint({ "palette" }, "commands"),
			press = true,
			tip = "Find a command",
			priority = 2,
		}
	end
	-- (In the form of a new issue, while a title or a comment is written, and in the words for the agent
	-- of Work on, `?` is a character of the words, not the card of keys.)
	if not S.new_open and not S.editing and not S.writing and not work.typing() then
		finish[#finish + 1] = {
			key = "help",
			words = hint({ "help" }, "keys"),
			press = true,
			tip = "Show the keys",
			priority = 9,
		}
	end
	return finish
end

-- What the keyboard does in the form that hands issues to an agent: Tab goes on, and on a choice the
-- arrows choose (in the words for the agent they move the caret).
local function work_hints()
	local start = { { key = "next_field", words = hint({ "next_field" }, "next field"), priority = 8 } }
	if not work.typing() then
		start[#start + 1] = {
			key = "choose",
			words = keyed({ "left", "right" }, "choose"),
			priority = 6,
		}
	end
	return start
end

-- What the keyboard does in the form of a new issue: Tab goes on, and on a value Enter chooses.
local function new_hints()
	local start = { { key = "next_field", words = hint({ "next_field" }, "next field"), priority = 8 } }
	if new.on_value() then
		start[#start + 1] = { key = "choose", words = hint({ "open" }, "choose"), priority = 6 }
	end
	return start
end

-- What the keyboard does while the title of the issue is written: it goes in, or it does not.
local function edit_hints()
	return {
		{ key = "save", words = keyed({ "enter" }, "save"), priority = 8 },
		{ key = "cancel", words = keyed({ "escape" }, "cancel"), priority = 7 },
	}
end

-- What the keyboard does while the card of the values is open (the labels, a filter): the arrows move,
-- Enter turns the value on or off and the card stays, Esc puts it away. The commands wait until it is
-- closed: the palette's and the card of keys' are not said.
local function picker_hints()
	local function keys(names, words)
		local row = b.div():row():items_center():gap(5):no_shrink()
		for _, name in names do
			row:child(u:kbd(name))
		end
		return row:child(b.text(words))
	end
	return {
		{ key = "move", words = keys({ "up", "down" }, "move"), priority = 8 },
		{ key = "turn", words = keys({ "enter" }, "turn on or off"), priority = 7 },
		{ key = "close", words = keys({ "escape" }, "close"), priority = 6 },
	}
end

-- What the keyboard does while a comment is written: it goes in, it is posted, or it is left. Posting is
-- on the Comment button, with its key (a key is written once), so the line says only how it is left.
local function writing_hints()
	return {
		{ key = "leave", words = keyed({ "escape" }, "leave"), priority = 8 },
	}
end

-- What the keyboard does on the chips of the filters: it goes on, it opens the menu, it goes back to the
-- list.
local function filter_hints()
	return {
		{ key = "next_chip", words = hint({ "next_field" }, "next"), priority = 8 },
		{ key = "choose_chip", words = hint({ "open" }, "choose"), priority = 7 },
		{ key = "leave_chips", words = hint({ "back" }, "back to the list"), priority = 6 },
	}
end

-- What the keyboard does while words are typed in the field and some are there: the arrows and Enter
-- work on the list (said first, as the list's keys are, where there is a line to move on and the list
-- is not behind the issue), all of Linear can be searched for the words (where the page says so in a
-- button of its own, the key is on the button), and Esc clears them, or leaves the search.
local function typing_hints()
	local start = {}
	if list.count() > 0 and not S.alone then
		start[#start + 1] = { key = "move", words = hint({ "up", "down" }, "move"), priority = 6 }
		start[#start + 1] = { key = "open", words = hint({ "open" }, "open"), priority = 5 }
	end
	if not head.unmatched() then
		start[#start + 1] = { key = "search", words = hint({ "search" }, "search all of Linear"), priority = 8, press = true }
	end
	start[#start + 1] = {
		key = "clear",
		words = hint({ "back" }, S.scope == "search" and "back" or "clear"),
		priority = 7,
	}
	return start
end

-- The values the bar changes, in the order the keys are in the card: the kind of choice (`change`'s),
-- the command, its words.
local VALUES = {
	{ "state", "status", "Status" },
	{ "priority", "priority", "Priority" },
	{ "assignee", "assignee", "Assignee" },
	{ "labels", "labels", "Labels" },
	{ "project", "project", "Project" },
	{ "cycle", "cycle", "Cycle" },
}

-- Which of them the pane has room for: all of them, the first four, or the status and the assignee.
local function shown_values()
	if S.width >= SIZE.bar_all then
		return VALUES
	elseif S.width >= SIZE.bar_four then
		return { VALUES[1], VALUES[2], VALUES[3], VALUES[4] }
	end
	return { VALUES[1], VALUES[3] }
end

-- Whether the bar is the line under the page: issues are selected and the list is seen (where the issue
-- has the whole pane what is selected is not seen, and a change is of the issue read), and no form or
-- title is where the keys are.
function status.selecting()
	return list.marked() > 0
		and not S.alone
		and not S.work_open
		and not S.new_open
		and not S.editing
		and not S.writing
end

-- The bar of the selection, in the place of the line of keys and as high. How many are selected is
-- said inside a place of its own width, so that the buttons do not move as the count does.
function status.bar()
	local t = u.theme
	local bar = b.div():id("selection-bar"):row():items_center():gap(6):h(t.size.status):w_full():no_shrink():bg(t.surface):px(8)
	bar:child(
		b.div():w(84):no_shrink():child(
			b.text(list.marked() .. " selected"):text_size(SIZE.tiny + 0.5):weight(700):color("accent"):no_shrink()
		)
	)
	for _, value in shown_values() do
		-- (Each button stands in a place of its own name, which a card of its values stands by: the
		-- button cannot be pressed while a change is on its way, and the place is there all the same.)
		bar:child(b.div():id("bar-place:" .. value[1]):no_shrink():child(kit.button("bar:" .. value[1], value[3], {
			bar = true,
			suffix = commands.label(value[2]),
			disabled = not change.can(value[1] == "priority")
				or (value[1] == "cycle" and not change.offers("cycle", change.targets())),
			tip = "Change the " .. string.lower(value[3]) .. " of the issues selected",
		})))
	end
	bar:child(b.spacer())
	bar:child(kit.button("bar:more", "More", {
		bar = true,
		suffix = commands.label("palette"),
		tip = "Every change, for the issues selected",
	}))
	bar:child(kit.button("clear-selection", "Clear", { bar = true, suffix = "Esc", tip = "Let go of the issues selected" }))
	return bar
end

-- A button of the bar pressed. The reply, or nil where it was not the bar's.
function status.press(id)
	local kind = string.match(id, "^bar:(%a+)$")
	if kind == nil then
		return nil
	elseif kind == "more" then
		return act.run("palette")
	end
	return change.run(kind, "bar-place:" .. kind)
end

function status.view()
	if S.screen == "list" and pictures.is_open() then
		-- A picture is large: a key that has a button (Esc, the arrows, find, save, the matches) is written
		-- on the button and not here; what is said here is what no button has: copying what is selected,
		-- and selecting all the words.
		local start = {}
		if not pictures.find_open() then
			if pictures.selected() then
				start[#start + 1] = { key = "copy", words = hint({ "pdf_copy" }, "copy"), priority = 8 }
			end
			if pictures.readable() then
				start[#start + 1] = { key = "all", words = hint({ "pdf_select_all" }, "select all"), priority = 6 }
			end
		end
		return line({ start = start })
	end
	-- (The bar of the selection stays while a card of its values is open: what is selected and the
	-- button the card stands by do not go.)
	if S.screen == "list" and picker.is_open() and not status.selecting() then
		return u:status("status", { start = picker_hints() })
	end
	if S.editing then
		return line({ start = edit_hints(), finish = status.finish() })
	end
	if S.writing then
		return line({ start = writing_hints(), finish = status.finish() })
	end
	if S.new_open then
		return line({ start = new_hints(), finish = status.finish() })
	end
	if S.work_open then
		return line({ start = work_hints(), finish = status.finish() })
	end
	if status.selecting() then
		return status.bar()
	end
	if filters.on_chip() then
		return line({ start = filter_hints(), finish = status.finish() })
	end
	if head.typing() and head.words() ~= "" then
		return line({ start = typing_hints(), finish = status.finish() })
	end
	-- A list with no line has nothing to move on, select or open: its keys are not said. Nor is a
	-- key said where it does something else: in the filter `x` is a letter, and where the issue has
	-- the whole pane there is no line to select or to open.
	local start = {}
	local typing = head.typing()
	local alone = S.alone
	if alone then
		-- The issue has the pane: the arrows are its own, and `j` and `k` go to the other issues.
		start[#start + 1] = { key = "scroll", words = hint({ "up", "down" }, "scroll"), priority = 8 }
		if list.count() > 1 then
			start[#start + 1] = {
				key = "next",
				words = keyed({ "j", "k" }, "next issue"),
				priority = 7,
			}
		end
	elseif list.count() > 0 then
		start[#start + 1] = { key = "move", words = hint({ "up", "down" }, "move"), priority = 8 }
		if not typing and not alone then
			start[#start + 1] = { key = "select", words = hint({ "select" }, "select"), priority = 6 }
		end
		if not alone then
			start[#start + 1] = { key = "open", words = hint({ "open" }, "open"), priority = 7 }
		end
	end
	if S.panel_open and not typing and not alone then
		start[#start + 1] = { key = "read", words = hint({ "read_down" }, "scroll"), priority = 5 }
	end
	-- The keys that change the issues are in the card and the palette; this one says there are some.
	if not typing and commands.when.changeable() then
		start[#start + 1] = { key = "status", words = hint({ "status" }, "status"), priority = 4 }
	end
	-- (Where the issue has the pane the list is not there to go from: its keys are not said.)
	if not typing and not alone and commands.enabled(commands.get("lists")) then
		start[#start + 1] = { key = "lists", words = hint({ "lists" }, "go to"), priority = 5 }
	end
	if not typing and not alone and commands.enabled(commands.get("filters")) then
		start[#start + 1] = { key = "filters", words = hint({ "filters" }, "filter"), priority = 3, press = true }
	end
	-- (Where the issue has the pane, Esc closes it: the search and the filter are the list's.)
	if not alone and S.scope == "search" then
		start[#start + 1] = { key = "clear", words = hint({ "back" }, "back"), priority = 5 }
	elseif not alone and head.field.value ~= "" then
		start[#start + 1] = { key = "clear", words = hint({ "back" }, "clear the filter"), priority = 5 }
	end
	-- A key that has a button is on the button; where the head has none (the list has too little room:
	-- the pane is narrow, or an issue stands beside it) the key is here, which is the only place it is
	-- written.
	if S.room < SIZE.wide then
		start[#start + 1] = { key = "refresh", words = hint({ "refresh" }, "refresh"), priority = 3 }
	end
	if S.room < SIZE.roomy and not typing then
		start[#start + 1] = { key = "new", words = hint({ "new" }, "new issue"), priority = 4 }
	end
	-- The marks of the agents on the lines are pressed, and have no key: the key goes to the agent of the
	-- one issue the keys are for, and is said here, with its name, where that issue has one. Last, so that
	-- nothing before it moves when the cursor comes to such an issue or leaves it.
	local reached = not typing and commands.when.reachable() and work.agent_of(change.single()) or nil
	if reached then
		start[#start + 1] = {
			key = "go_to_agent",
			words = hint({ "go_to_agent" }, "go to " .. reached.agent),
			priority = 6,
			press = true,
		}
	end
	local finish = status.finish()
	return line({ start = start, finish = finish })
end

-- An item of the line pressed: the command it stands for. Whether it was the line's.
function status.action(action)
	if action.from ~= "status" or action.kind ~= "press" then
		return false
	end
	if
		action.key == "palette"
		or action.key == "help"
		or action.key == "filters"
		or action.key == "search"
		or action.key == "go_to_agent"
	then
		act.run(action.key)
	end
	return true
end

-- The line of a screen that has no key to say.
function status.empty()
	return line({})
end

-- The line of the Connect screen: where the keyboard goes next.
function status.connect()
	return line({ start = { { key = "next", words = keyed({ "tab" }, "next"), priority = 5 } } })
end

return status
