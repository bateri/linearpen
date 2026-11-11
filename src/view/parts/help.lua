-- The card of keys: every command with the keys it has, in groups, from the same table as the palette
-- and the status line. A modal over the page; Esc closes it.

local kit = require("../lib/kit")
local commands = require("./commands")

local b, u, S, act = kit.b, kit.u, kit.S, kit.act

local help = {}

local H = { open = false }

-- The groups of the card, in the order they stand in the table.
local ORDER = { "Move", "Select", "Issue", "Work on", "Everywhere" }

function help.is_open()
	return H.open
end

function help.show()
	H.open = true
end

function help.hide()
	H.open = false
end

local function label(words)
	return b.text(string.upper(words)):text_size(11.5):color("dim")
end

-- The keys of a command, as caps, and its words. Two keys at most: the rest are in the table.
local function line(command)
	local row = b.div():row():items_center():gap(8):h(22):no_shrink()
	for index, words in command.keys do
		if index > 2 then
			break
		end
		row:child(u:kbd(words))
	end
	row:child(b.text(command.title):text_size(13):truncate():min_w(0):grow(1))
	return row
end

local function group(name)
	local box = b.div():col():gap(7):mb(18)
	box:child(label(name))
	for _, command in commands.all() do
		if command.group == name then
			box:child(line(command))
		end
	end
	return box
end

function help.view(root)
	if not H.open then
		return
	end
	local width = math.min(640, S.width - 32)
	local inner = width - 36
	local body = b.div():col():p(16):pb(0):w(width)
	local head = b.div():row():items_center():gap(8):mb(16):w(inner)
	head:child(b.text("Keys"):text_size(15):weight(700))
	head:child(b.spacer())
	head:child(kit.button("help-close", "Close", { suffix = "Esc" }))
	body:child(head)
	local columns = b.div():row():items_start():gap(28):w(inner)
	if width >= 520 then
		-- The left groups have short words; the room they leave is the right's, whose longest line
		-- is whole in a pane wide enough for the card.
		local left_width = 210
		local left = b.div():col():w(left_width):no_shrink()
		local right = b.div():col():w(inner - 28 - left_width):no_shrink()
		for index, name in ORDER do
			local side = index <= 2 and left or right
			side:child(group(name))
		end
		columns:child(left):child(right)
	else
		local single = b.div():col():w(inner):no_shrink()
		for _, name in ORDER do
			single:child(group(name))
		end
		columns:child(single)
	end
	body:child(b.div():id("help/scroll"):scroll():max_h(math.max(S.height - 160, 160)):w(inner):child(columns))
	root:child(u:modal("help", { width = width, label = "the keys", top = 40 }, body))
end

function help.action(action)
	if action.from == "help" and action.kind == "dismiss" then
		H.open = false
		return true
	end
	return nil
end

act.open_help = help.show

return help
