-- The line of keys under the page (the library's `u:status`): what the keyboard does here, each
-- key said once. A key that has a button on the page is on the button, so Refresh's key is here only
-- where the pane is too narrow for the button.

local kit = require("../lib/kit")
local commands = require("./commands")

local b, u, S, SIZE = kit.b, kit.u, kit.S, kit.SIZE

local status = {}

-- The keys of a hint, as the library draws them, and its words.
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

function status.view()
	local start = {
		{ key = "move", words = hint({ "up", "down" }, "move"), priority = 8 },
		{ key = "fold", words = hint({ "fold", "unfold" }, "fold"), priority = 6 },
		{ key = "scope", words = hint({ "scope_mine", "scope_all" }, "scope"), priority = 5 },
	}
	if S.width < SIZE.wide then
		start[#start + 1] = { key = "refresh", words = hint({ "refresh" }, "refresh"), priority = 4 }
	end
	return u:status("status", { start = start })
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
