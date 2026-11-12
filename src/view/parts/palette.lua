-- The palette: a field over every command that can be done now (with its keys), and what is typed
-- as the words of a filter: `Filter issues by "bug"`. From the same table as the card of keys and the
-- status line.

local fuzzy = require("@pencere/fuzzy")
local kit = require("../lib/kit")
local commands = require("./commands")
local head = require("./head")

local u, act = kit.u, kit.act

local palette = {}

local P = { open = false, items = nil, state = nil, serial = 0 }
palette.state = P

local LIMIT = 40

-- What the person typed as the words of the filter.
local function as_filter(query)
	return { key = "go:" .. query, title = string.format("Filter issues by “%s”", query), hint = "Issues", text = query }
end

-- The page's own way of finding: the commands that fuzzily match, then the query as a filter.
local function found(query, items)
	local rows = {}
	if query == "" then
		for index = 1, math.min(#items, 200) do
			rows[#rows + 1] = items[index]
		end
		return rows
	end
	for _, match in fuzzy.filter(query, items, { text = "title", limit = LIMIT }) do
		rows[#rows + 1] = match.item
	end
	rows[#rows + 1] = as_filter(query)
	return rows
end

function palette.is_open()
	return P.open
end

function palette.show()
	-- The commands that can be done now, made as the palette opens (while the page has the keyboard,
	-- which is what most of them need) and kept until it closes.
	P.items = commands.items()
	-- A palette of its own each time: the host keeps the text of a field of the same name, and one
	-- opened again would begin with the words of the last.
	P.serial += 1
	P.state = { query = "", cursor = nil, field = { value = "" } }
	P.open = true
end

function palette.hide()
	P.open = false
end

function palette.view(root)
	if not P.open then
		return
	end
	root:child(u:palette("palette" .. P.serial, {
		items = P.items,
		state = P.state,
		label = "Command palette",
		hint = "Find a command, or filter the issues",
		filter = found,
		rows = kit.card_rows(),
		footer = {
			{ keys = "enter", title = "Do it" },
			{ keys = "escape", title = "Close" },
		},
	}))
end

-- An action of the palette: whether it was one, and its reply.
function palette.action(action)
	if type(action.from) ~= "string" or string.match(action.from, "^palette%d+$") == nil then
		return nil
	end
	if action.kind == "dismiss" then
		P.open = false
		return true
	elseif action.kind == "query" then
		-- The best match is the one the cursor goes to, as the query changes.
		P.state.cursor = nil
		return true
	elseif action.kind == "pick" then
		P.open = false
		local item = action.item
		if item.text ~= nil then
			head.set(item.text)
			return true
		end
		return act.run(item.key)
	end
	return true
end

return palette
