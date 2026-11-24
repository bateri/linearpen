-- A pull request of the issue open, as GitHub tells it (`pull`, read with `gh`), in the place of the
-- issue beside the list: Back and Open on GitHub over it, where it is and its state, its title, who
-- made it and from which branch to which; then its checks, its review and its files. What the issue
-- told of it (where it is, its number and its title) is drawn at once; what GitHub tells comes into
-- its place under them, so nothing over it moves when the answer comes. Read only: nothing is
-- reviewed, commented or merged from here.

local kit = require("../lib/kit")
local commands = require("./commands")
local icons = require("./icons")

local b, u, SIZE = kit.b, kit.u, kit.SIZE

local pull = {}

-- The box that scrolls (the issue's: Space and Shift Space scroll it as they scroll the issue).
local SCROLL = "panel-scroll"

-- Each state's word and the role of the theme it is drawn in.
local STATES = {
	open = { "Open", "success" },
	draft = { "Draft", "dim" },
	merged = { "Merged", "magenta" },
	closed = { "Closed", "error" },
}

-- What a review says, its words and its role.
local REVIEWS = {
	approved = { "Approved", "success" },
	changesRequested = { "Changes requested", "error" },
	commented = { "Commented", "dim" },
	dismissed = { "Dismissed", "dim" },
}

-- The room of a count of lines (`+120`, `−12`), and of the bar of five that says how they share a file.
local COUNT = 44
local BLOCKS = 5
local BLOCK = 7

-- The head of a section: its name as the issue's sections are named (`COMMENTS 2`), and what it counts.
local function heading(name, count)
	local row = b.div():row():items_center():gap(8):mt(20):mb(6):no_shrink()
	row:child(b.text(name):text_size(SIZE.tiny):color("dim"))
	if count ~= nil then
		row:child(b.text(count):text_size(SIZE.tiny):color("dim"))
	end
	return row
end

-- A line of a section: a picture in a column of its own, and words.
local function line(picture, words, color)
	return b.div():row():items_center():gap(8):h(22):no_shrink():child(
		b.div():w(14):no_shrink():child(picture)
	):child(b.text(words):text_size(SIZE.small):color(color or "foreground"):truncate():min_w(0))
end

-- The state, by its word and its colour together, in a pill.
local function state_pill(state)
	local said = STATES[state] or STATES.open
	local color = said[2]
	local picture = state == "closed" and icons.cross(color) or icons.pull_request(color)
	return b.div()
		:id("pull-state")
		:row()
		:items_center()
		:gap(6)
		:h(22)
		:px(9)
		:rounded(11)
		:no_shrink()
		:bg(b.alpha(color, 0.14))
		:border(1, b.alpha(color, 0.45))
		:motion({ seconds = u.theme.motion, color = true })
		:label("State " .. said[1])
		:child(picture)
		:child(b.text(said[1]):text_size(SIZE.small):weight(700):color(color))
end

-- The checks: how many passed, failed, run and were skipped; then the ones that failed and the ones
-- still running, by name (those that passed are counted, not listed).
local function checks(data)
	local c = data.checks
	local box = b.div():col():no_shrink()
	local total = c.passed + #c.failed + #c.running + c.skipped
	box:child(heading("CHECKS", total > 0 and tostring(total) or nil))
	if total == 0 then
		return box:child(b.text("No checks"):text_size(SIZE.small):color("dim"))
	end
	local summary = b.div():row():items_center():gap(14):wrap():no_shrink():mb(4)
	local function part(picture, words, color)
		summary:child(b.div():row():items_center():gap(5):no_shrink():child(picture):child(
			b.text(words):text_size(SIZE.small):color(color)
		))
	end
	if #c.failed > 0 then
		part(icons.cross(), tostring(#c.failed) .. " failed", "error")
	end
	if #c.running > 0 then
		part(kit.spinner(12, "warning"), tostring(#c.running) .. " running", "warning")
	end
	if c.passed > 0 then
		part(icons.tick(), tostring(c.passed) .. " passed", "success")
	end
	if c.skipped > 0 then
		part(icons.skip(), tostring(c.skipped) .. " skipped", "dim")
	end
	box:child(summary)
	for _, name in c.failed do
		box:child(line(icons.cross(), name))
	end
	for _, name in c.running do
		-- (Still running: its picture turns, by the terminal, until the next answer says it ended.)
		box:child(line(kit.spinner(12, "warning"), name))
	end
	return box
end

-- The review: what GitHub decided, the last review of each who reviewed, and who is waited for.
local function review(data)
	local r = data.review
	local box = b.div():col():no_shrink()
	box:child(heading("REVIEW"))
	local decided = {
		approved = { icons.tick(), "Approved", "success" },
		changesRequested = { icons.cross(), "Changes requested", "error" },
		required = { icons.ring("warning"), "Review required", "warning" },
	}
	local said = decided[r.decision]
	if said ~= nil then
		box:child(line(said[1], said[2], said[3]))
	elseif #r.reviews == 0 then
		box:child(b.text("No review asked"):text_size(SIZE.small):color("dim"))
	end
	for _, one in r.reviews do
		local words = REVIEWS[one.state] or REVIEWS.commented
		box:child(
			b.div():row():items_center():gap(8):h(22):no_shrink():child(b.div():w(14):no_shrink()):child(
				b.text(one.who):text_size(SIZE.small):weight(700):truncate():min_w(0)
			):child(b.text(words[1]):text_size(SIZE.small):color(words[2]):no_shrink())
		)
	end
	if #r.waiting > 0 then
		box:child(line(icons.ring(), "Waiting for " .. table.concat(r.waiting, ", "), "dim"))
	end
	return box
end

-- The bar of a file: five blocks, as many coloured as its lines are of those of the file that changes
-- most (at least one where it changes any), shared between what it adds and what it takes away; the
-- rest grey.
local function bar(file, most)
	local total = file.additions + file.deletions
	local filled = total > 0 and math.max(1, math.ceil(BLOCKS * total / math.max(most, 1))) or 0
	filled = math.min(filled, BLOCKS)
	local green = total > 0 and math.floor(filled * file.additions / total + 0.5) or 0
	local row = b.div():row():gap(2):no_shrink():w(BLOCKS * BLOCK + (BLOCKS - 1) * 2)
	for index = 1, BLOCKS do
		local color = index <= green and "success" or index <= filled and "error" or b.alpha("foreground", 0.15)
		row:child(b.div():size(BLOCK, BLOCK):rounded(2):bg(color))
	end
	return row
end

-- A count of lines, `+120` or `−12`, right in a column of its own; a 0 is dim.
local function count(sign, n, color)
	return b.div():w(COUNT):no_shrink():row():justify_end():child(
		b.text(sign .. tostring(n)):text_size(SIZE.small):color(n > 0 and color or "dim")
	)
end

-- The files: how many and how many lines, then each (its name whole, its folder after it, cut first),
-- not pressable (the lines of a file are on GitHub).
local function files(data)
	local box = b.div():col():no_shrink()
	local head = heading("FILES", tostring(data.changedFiles))
	head:child(b.spacer())
	head:child(b.text("+" .. tostring(data.additions)):text_size(SIZE.small):color("success"))
	head:child(b.text("\u{2212}" .. tostring(data.deletions)):text_size(SIZE.small):color("error"))
	box:child(head)
	local most = 0
	for _, file in data.files do
		most = math.max(most, file.additions + file.deletions)
	end
	for _, file in data.files do
		local folder, name = string.match(file.path, "^(.*/)([^/]+)$")
		folder, name = folder or "", name or file.path
		box:child(
			b.div():row():items_center():gap(10):h(22):no_shrink():label(
				string.format("%s, %d added, %d taken away", file.path, file.additions, file.deletions)
			):child(bar(file, most)):child(count("+", file.additions, "success")):child(
				count("\u{2212}", file.deletions, "error")
			):child(b.text(name):text_size(SIZE.small):no_shrink()):child(
				b.text(folder):text_size(SIZE.small):color("dim"):truncate():min_w(0)
			)
		)
	end
	local more = data.changedFiles - #data.files
	if more > 0 then
		box:child(
			b.text(string.format("And %d more on GitHub", more)):text_size(SIZE.small):color("dim"):mt(6)
		)
	end
	return box
end

-- What stands under the head while GitHub is asked, where it did not tell, and when it did.
local function body(Pl)
	if Pl.status == "loading" then
		return b.div():row():items_center():gap(8):mt(16):no_shrink():child(kit.spinner(13, "dim")):child(
			b.text("Asking GitHub"):text_size(SIZE.small):color("dim")
		)
	elseif Pl.status == "failed" then
		return b.div():col():gap(10):mt(16):items_start():no_shrink():child(
			b.div():row():items_start():gap(8):child(b.div():size(7, 7):rounded(4):bg("warning"):no_shrink():mt(6)):child(
				b.text(Pl.error or "GitHub did not tell it"):text_size(SIZE.small):color("warning"):wrap():min_w(0)
			)
		):child(kit.button("pull-retry", "Try again"))
	end
	-- (It comes in as it comes, fading: what is over it stays where it was.)
	return b.div()
		:id("pull-body:" .. Pl.url)
		:col()
		:no_shrink()
		:motion({ enter = { opacity = 0, seconds = 0.24 } })
		:child(checks(Pl.data))
		:child(review(Pl.data))
		:child(files(Pl.data))
end

-- The pull request `Pl` (`{ url, number, title, repository, status, data, error }`), `width` points wide.
function pull.view(width, Pl)
	local data = Pl.data
	local top = b.div():col():no_shrink():pt(SIZE.head_top):px(SIZE.pad):pb(14)
	top:child(
		b.div():row():items_center():gap(8):no_shrink():child(
			kit.button("pull-back", "Back", { suffix = "Esc", tip = "Back to the issue" })
		):child(b.spacer()):child(kit.button("pull-github", "Open on GitHub", {
			suffix = commands.label("pull_github"),
			tip = "Open the pull request in your browser",
		}))
	)
	-- Where it is and its number, at once; its state when GitHub tells it, at the end of the line.
	local where = b.div():row():items_center():gap(8):mt(9):h(22):no_shrink():child(
		b.text(Pl.repository .. " #" .. tostring(Pl.number)):text_size(SIZE.small):color("dim"):truncate():min_w(0)
	):child(b.spacer())
	if data ~= nil then
		where:child(state_pill(data.state))
	end
	top:child(where)
	top:child(b.text(data ~= nil and data.title or Pl.title):text_size(16):weight(700):wrap():mt(3):no_shrink())
	-- Who made it and from which branch to which: a line kept for it, so that its words come into it.
	local by = b.div():row():items_center():gap(6):mt(6):h(18):no_shrink()
	if data ~= nil then
		if data.author ~= nil then
			by:child(b.text(data.author):text_size(SIZE.small):weight(700):no_shrink())
		end
		by:child(b.text(data.head):text_size(SIZE.small):color("dim"):truncate():min_w(0))
		by:child(b.text("\u{2192}"):text_size(SIZE.small):color("dim"):no_shrink())
		by:child(b.text(data.base):text_size(SIZE.small):color("dim"):no_shrink())
	end
	top:child(by)
	local content = b.div():col():grow(1):min_w(0):min_h(0):child(top)
	content:child(b.div():h(1):bg(b.alpha("foreground", 0.18)):no_shrink())
	content:child(b.div():id(SCROLL):grow(1):min_h(0):scroll():col():pt(2):px(SIZE.pad):pb(SIZE.pad):child(body(Pl)))
	return b.div():row():w(width):no_shrink():min_h(0):child(content)
end

return pull
