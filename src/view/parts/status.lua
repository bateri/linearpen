-- The line under the page. With issues selected it is the bar of the selection (how many, the values
-- to change, More and Clear), as high as the line of keys that it stands in the place of; else it is
-- the line of keys (the library's `u:status`): what the keyboard does here, each key said once. A
-- key that has a button on the page is on the button, so the keys of Refresh and New issue are here only
-- where the pane is too narrow for their buttons. The two at the end of the line open the palette and
-- the card of keys, which hold the rest.

local kit = require("../lib/kit")
local commands = require("./commands")
local change = require("./change")
local head = require("./head")
local list = require("./list")
local new = require("./new")

local b, u, S, SIZE, act = kit.b, kit.u, kit.S, kit.SIZE, kit.act

local status = {}

-- The keys of a hint, as the library draws them, and its words. `ids` are commands, `words` what
-- they are said to do.
local function hint(ids, words)
	local row = b.div():row():items_center():gap(5):no_shrink()
	for _, id in ids do
		local first = commands.first(id)
		if first ~= nil then
			row:child(u:kbd(first))
		end
	end
	return row:child(b.text(words))
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
	-- (In the form of a new issue, and while a title is written, `?` is a character of the words, not the
	-- card of keys.)
	if not S.new_open and not S.editing then
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

-- What the keyboard does in the form that hands issues to an agent.
local function work_hints()
	return {
		{ key = "next_field", words = hint({ "next_field" }, "next field"), priority = 8 },
		{
			key = "choose",
			words = b.div():row():items_center():gap(5):no_shrink():child(u:kbd("left")):child(u:kbd("right")):child(
				b.text("choose")
			),
			priority = 6,
		},
	}
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
		{ key = "save", words = b.div():row():items_center():gap(5):no_shrink():child(u:kbd("enter")):child(b.text("save")), priority = 8 },
		{ key = "cancel", words = b.div():row():items_center():gap(5):no_shrink():child(u:kbd("escape")):child(b.text("cancel")), priority = 7 },
	}
end

-- The values the bar changes, in the order the keys are in the card: the kind of choice (`change`'s),
-- the command, its words.
local VALUES = {
	{ "state", "status", "Status" },
	{ "priority", "priority", "Priority" },
	{ "assignee", "assignee", "Assignee" },
	{ "labels", "labels", "Labels" },
	{ "project", "project", "Project" },
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
	return list.marked() > 0 and not S.alone and not S.work_open and not S.new_open and not S.editing
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
		bar:child(kit.button("bar:" .. value[1], value[3], {
			bar = true,
			suffix = commands.label(value[2]),
			disabled = not change.can(value[1] == "priority"),
			tip = "Change the " .. string.lower(value[3]) .. " of the issues selected",
		}))
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
	return change.run(kind, id)
end

function status.view()
	if S.editing then
		return u:status("status", { start = edit_hints(), finish = status.finish() })
	end
	if S.new_open then
		return u:status("status", { start = new_hints(), finish = status.finish() })
	end
	if S.work_open then
		return u:status("status", { start = work_hints(), finish = status.finish() })
	end
	if status.selecting() then
		return status.bar()
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
				words = b.div():row():items_center():gap(5):no_shrink():child(u:kbd("j")):child(u:kbd("k")):child(
					b.text("next issue")
				),
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
	if head.field.value ~= "" then
		start[#start + 1] = { key = "clear", words = hint({ "back" }, "clear the filter"), priority = 5 }
	end
	-- A key that has a button is on the button; where the head has none (the page has too little room)
	-- the key is here, which is the only place it is written.
	if S.width < SIZE.wide then
		start[#start + 1] = { key = "refresh", words = hint({ "refresh" }, "refresh"), priority = 3 }
	end
	if S.width < SIZE.roomy and not typing then
		start[#start + 1] = { key = "new", words = hint({ "new" }, "new issue"), priority = 4 }
	end
	local finish = status.finish()
	return u:status("status", { start = start, finish = finish })
end

-- An item of the line pressed: the command it stands for. Whether it was the line's.
function status.action(action)
	if action.from ~= "status" or action.kind ~= "press" then
		return false
	end
	if action.key == "palette" or action.key == "help" then
		act.run(action.key)
	end
	return true
end

-- The line of a screen that has no key to say.
function status.empty()
	return u:status("status", {})
end

-- The line of the Connect screen: where the keyboard goes next.
function status.connect()
	local row = b.div():row():items_center():gap(5):no_shrink():child(u:kbd("tab")):child(b.text("next"))
	return u:status("status", { start = { { key = "next", words = row, priority = 5 } } })
end

return status
