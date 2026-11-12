-- The card of lists, which the button of the head and `g` open: the lists a person can go to, one a
-- line with what it is and, where it has one, its key. It is the kit's palette (a field that finds,
-- the arrows, Enter, Esc). The lists are My issues and All issues; the program holds what each shows.

local kit = require("../lib/kit")

local u, S, act = kit.u, kit.S, kit.act

local lists = {}

-- What a list is, and the key that goes to it without the card.
local ITEMS = {
	{ key = "mine", title = "My issues", hint = "Assigned to you", keys = "1" },
	{ key = "all", title = "All issues", hint = "The workspace", keys = "2" },
}

local L = { open = false, state = nil, serial = 0 }
lists.state = L

function lists.is_open()
	return L.open
end

-- Opens the card, the cursor on the list that is shown. A card of its own each time: the host keeps
-- the text of a field of the same name, and one opened again would begin with the words of the last.
function lists.show()
	L.serial += 1
	L.state = { query = "", cursor = S.scope, field = { value = "" } }
	L.open = true
end

function lists.hide()
	L.open = false
end

function lists.view(root)
	if not L.open then
		return
	end
	root:child(u:palette("lists" .. L.serial, {
		items = ITEMS,
		state = L.state,
		label = "Lists",
		hint = "Go to a list",
		rows = 10,
		footer = {
			{ keys = "enter", title = "Open" },
			{ keys = "escape", title = "Close" },
		},
	}))
end

-- What the person did to the card. Whether it was the card's, and the reply.
function lists.action(action)
	if type(action.from) ~= "string" or string.match(action.from, "^lists%d+$") == nil then
		return nil
	end
	if action.kind == "dismiss" then
		L.open = false
	elseif action.kind == "query" then
		L.state.cursor = nil
	elseif action.kind == "pick" then
		L.open = false
		act.view({ scope = action.item.key })
	end
	return true
end

return lists
