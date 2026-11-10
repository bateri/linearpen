-- The page of linearpen. The program asks Linear and tells the page what it needs by the calls and
-- topics of docs/calls.md; the list is not drawn yet, so the pane says what it is waiting for.

local b = pencere

local function render(size, now, reduce_motion)
	return b.div():size_full():col():items_center():justify_center():gap(8):bg("background"):family("terminal")
		:child(b.text("Linear"):weight(650))
		:child(b.text("The issues are not drawn yet"):color("dim"))
end

local function handle(event)
	return nil
end

return { render = render, handle = handle }
