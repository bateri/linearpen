-- What the page says where there is no list to show: the Connect screen (no key, or one Linear did
-- not accept), the skeleton of the list while it is first asked for, the error where Linear cannot
-- be reached, and the empty lists.

local kit = require("../lib/kit")
local commands = require("./commands")
local lists = require("./lists")

local b, u, S, SIZE = kit.b, kit.u, kit.S, kit.SIZE

local screens = {}

-- The page of Linear where a key is made.
screens.KEYS_URL = "https://linear.app/settings/account/security"

-- The state of the key's field.
local K = { value = "" }
screens.field = K

local function centered(card)
	return b.div():grow(1):min_h(0):row():items_center():justify_center():child(card)
end

-- Connect: a key is asked for, and told where one is made. `problem` says why the last was not
-- taken.
function screens.connect()
	local card = b.div():col():w(380):max_w(S.width - 32):gap(12):no_shrink()
	card:child(b.canvas({
		b.path():circle(14, 14, 10.8):stroke(2.4, b.alpha("magenta", 0.65)),
		b.path():move_to(9.4, 14.4):line_to(12.9, 17.9):line_to(19.7, 10.4):stroke(2.4, "magenta"),
	}):size(28, 28):no_shrink())
	card:child(b.text("Connect Linear"):text_size(16):weight(700))
	local snapshot = S.issues
	local refused = snapshot ~= nil and snapshot.error ~= nil and snapshot.error.kind == "refused" and snapshot.error.message or nil
	local why = S.connect_error or S.key_problem or refused
	local field = b.div():col():gap(6)
	field:child(b.text("API key"):text_size(SIZE.tiny):color("dim"))
	field:child(u:input("key", {
		state = K,
		label = "Linear API key",
		hint = "lin_api_",
		kind = "secret",
		tone = why ~= nil and "bad" or nil,
	}))
	card:child(field)
	if why ~= nil then
		card:child(b.text(why):text_size(SIZE.tiny):color("error"):wrap())
	end
	local actions = b.div():row():items_center():gap(8)
	actions:child(kit.button("connect", "Connect", {
		primary = true,
		suffix = "Enter",
		busy = S.connecting,
		disabled = S.connecting,
	}))
	actions:child(kit.button("create-key", "Create a key in Linear", { tip = screens.KEYS_URL }))
	card:child(actions)
	-- What the key may do: it shows the issues with Read access, makes new ones with Create issues and
	-- changes them with Write.
	card:child(
		b.div():col():gap(2):no_shrink():child(b.text("A key with Read access shows your issues."):text_size(SIZE.tiny):color("dim")):child(
			b.text("Create issues lets you make new ones."):text_size(SIZE.tiny):color("dim")
		):child(b.text("Write lets you change them."):text_size(SIZE.tiny):color("dim"))
	)
	return centered(card)
end

-- The list while it is first asked for: lines that stand where the issues will.
function screens.loading()
	local box = b.div():col():grow(1):min_h(0):clip()
	box:child(
		b.div():row():items_center():gap(10):pt(SIZE.head_top):pb(SIZE.head_bottom):px(SIZE.pad):h(SIZE.control + SIZE.head_top + SIZE.head_bottom):no_shrink():child(
			b.text(lists.name()):text_size(15):weight(700)
		):child(kit.spinner(13, "dim")):child(b.text("Loading"):text_size(SIZE.small):color("dim"))
	)
	local groups = { { 112, { 52, 38, 61, 44 } }, { 96, { 47, 58 } }, { 60, { 41, 66, 35 } } }
	for _, group in groups do
		box:child(
			b.div():h(SIZE.row + SIZE.group_gap):pt(SIZE.group_gap):px(18):row():items_center():child(
				b.div():w(group[1]):h(10):rounded(5):bg(b.alpha("foreground", 0.16))
			)
		)
		for _, width in group[2] do
			box:child(
				b.div():h(SIZE.row):pl(42):pr(18):row():items_center():gap(12):child(
					b.div():w(54):h(9):rounded(5):bg(b.alpha("foreground", 0.08)):no_shrink()
				):child(b.div():w_pct(width):h(9):rounded(5):bg(b.alpha("foreground", 0.1)))
			)
		end
	end
	return box
end

-- Linear could not be asked, or did not answer as it should: why, and the way to try again.
function screens.failed(err)
	local card = b.div():col():items_center():gap(10)
	card:child(b.text("Linear did not answer"):text_size(15):weight(700))
	card:child(b.text(err.message or "No more is known"):color("dim"):wrap():max_w(520):align("center"))
	card:child(b.div():row():gap(8):mt(6):child(kit.button("retry", "Try again", {
		primary = true,
		suffix = commands.label("refresh"),
		busy = S.refreshing,
		disabled = S.refreshing,
	})))
	return centered(card)
end

-- A list that is shown and has not come (the person went to it and Linear is being asked).
function screens.coming()
	local card = b.div():col():items_center():gap(10)
	card:child(kit.spinner(18, "quiet"))
	card:child(b.text("Asking Linear for " .. lists.name()):color("dim"):truncate():max_w(420))
	return centered(card)
end

-- Nothing in My issues: said once, with the way to the others.
function screens.empty()
	local card = b.div():col():items_center():gap(12)
	card:child(b.text("Nothing is assigned to you"):text_size(15):weight(700))
	card:child(kit.button("show-all", "Show all issues", { primary = true, suffix = commands.label("scope_all") }))
	return centered(card)
end

-- The list has no line: what Linear found, the words of the field or the filters let nothing through, or
-- there is nothing open. `o`: `words` (the field's), `search` (the list is a search), `narrowed` (filters
-- are set).
function screens.nothing(o)
	local card = b.div():col():items_center():gap(6)
	if o.search then
		card:child(b.text("Nothing found"):text_size(15):weight(700))
		card:child(b.text(o.words):color("dim"):truncate():max_w(420))
	elseif o.words ~= "" then
		-- The words match nothing held: all of Linear is the next place to look.
		card:child(b.text("Nothing in " .. lists.name() .. " matches"):text_size(15):weight(700):truncate():max_w(520))
		card:child(b.div():mt(6):child(kit.button("search-all", "Search all of Linear", {
			suffix = commands.label("search"),
			tip = "Search the whole workspace for these words",
		})))
	elseif o.narrowed then
		card:child(b.text("Nothing in " .. lists.name() .. " matches the filters"):text_size(15):weight(700):truncate():max_w(520))
	else
		card:child(b.text("No issues"):text_size(15):weight(700))
	end
	return centered(card)
end

return screens
