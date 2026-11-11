-- The line of keys under the page (the library's `u:status`): what the keyboard does here, each key
-- said once. A key that has a button on the page is on the button, so Refresh's key is here only
-- where the pane is too narrow for the button. The two at its end open the palette and the card of
-- keys, which hold the rest.

local kit = require("../lib/kit")
local commands = require("./commands")
local head = require("./head")
local list = require("./list")

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
	-- (In the form `?` is a character of the words for the agent, not the card of keys.)
	if not S.work_open then
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

function status.view()
	if S.work_open then
		return u:status("status", { start = work_hints(), finish = status.finish() })
	end
	local start = {
		{ key = "move", words = hint({ "up", "down" }, "move"), priority = 8 },
		{ key = "select", words = hint({ "select" }, "select"), priority = 6 },
	}
	if list.marked() > 0 then
		start[#start + 1] = {
			key = "select_more",
			words = b.div():row():items_center():gap(5):no_shrink():child(u:kbd("shift+up")):child(u:kbd("shift+down")):child(
				b.text("select more")
			),
			priority = 4,
		}
	end
	start[#start + 1] = { key = "open", words = hint({ "open" }, "open"), priority = 7 }
	if head.field.value ~= "" then
		start[#start + 1] = { key = "clear", words = hint({ "back" }, "clear the filter"), priority = 5 }
	end
	if S.room < SIZE.wide then
		start[#start + 1] = { key = "refresh", words = hint({ "refresh" }, "refresh"), priority = 3 }
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
