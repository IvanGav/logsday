let boardExists = false;
let board_height;
let heightOffset;
let board_width;
let widthOffset;
let alive_cells = new Map(); // Map[x,y] = state
let stepInterval;
let activeStepInterval = false;
let speed = 500;
let genSpeed = 25;
let satisfyingMode = false;
let points;
let activeScoringCategories = []; //list of {category: String, mult: number}
let displayScoreConditions = ["", "Free Round", ""];
let currentTurn = 0;

resetRules();
generateBoard();
resetEvolOrBorn();
listenKeys();

function regenerateBoard() {
    generateBoard();
    addRandRule();
}

function listenKeys() {
    document.body.addEventListener('keydown', function (event) {
        const key = event.key;

        switch (key) {
            case "ArrowLeft":
                if(!activeStepInterval) { moveBoard(-1, 0); }
                break;
            case "ArrowRight":
                if(!activeStepInterval) { moveBoard(1, 0); }
                break;
            case "ArrowUp":
                if(!activeStepInterval) { moveBoard(0, -1); }
                break;
            case "ArrowDown":
                if(!activeStepInterval) { moveBoard(0, 1); }
                break;
            case "Enter":   
                if (!activeStepInterval) {
                    doStep(); 
                    startSteps();
                } else {
                    stopSteps();
                }
                break;
            case " ":
                doStep();
                break;
        }
    });

}


function generateBoard(width=16, height=16) {
    if(boardExists) {
        document.getElementById("rulesContainer").remove();
    }
    boardExists = true;
    document.getElementById("genButton").style.display = "none";
    document.getElementById("stopButton").style.display = "flex";
    document.getElementById("goButton").style.display = "flex";
    document.getElementById("stepButton").style.display = "flex";
    document.getElementById("turboButton").style.display = "flex";
    document.getElementById("randButton").style.display = "flex";
    
    board_height = height;
    heightOffset = 0;
    board_width = width;
    widthOffset = 0;
    alive_cells = new Map(); // Map[x,y] = state
    points = 0;
    displayScoreConditions = ["", "Free Round", ""];
    currentTurn = 1;
    
    stopSteps();
    
    let gameDiv = document.getElementById("game");
    let board = document.createElement("div");
    board.id = "board";
    for(i = 0; i < width; i++) {
        let line = document.createElement("div");
        line.className = "line";
        for (let j = 0; j < height; j++) {
            let space = document.createElement("div");
            space.id = `${j},${i}`;
            space.onclick = function() { setStateWithElement(this, (Number(space.className.split("state-")[1]) + 1) % 5); };
            line.append(space);
            setTimeout(function () {
                space.className = "space pop-out state-0";
            }, i*genSpeed + j*genSpeed);
        }
        board.append(line);
    }
    let rulesPage = document.createElement("div");
    rulesPage.id = "rulesContainer";
    gameDiv.append(board);
    gameDiv.append(rulesPage);

    if (devmode) {

    } else {

    }
    let score = document.createElement("div");
    score.id = "score";
    rulesPage.append(score);
    let scoreHeaderDiv = document.createElement("div");
    scoreHeaderDiv.id = "scoreHeaderDiv";
    score.append(scoreHeaderDiv);
    let scoreTitle = document.createElement("h1");
    scoreTitle.innerHTML = "Score: 0";
    let turnTitle = document.createElement("h1");
    turnTitle.innerHTML = "Turn: 0";

    scoreHeaderDiv.append(turnTitle);
    scoreHeaderDiv.append(scoreTitle);

    let scoresPre = document.createElement("pre");
    scoresPre.id = "scorePre";
    score.append(scoresPre);

    let rules = document.createElement("div");
    rules.id = "rules";
    rulesPage.append(rules);
    putUpRules();
    assignNewScoringCategories();

    // resetEvolOrBorn();

    for(let i = 0; i < 4; i++) {
        let arrow = document.createElement("div");
        arrow.className = "arrow";
        if(i == 0) arrow.onclick = function() { moveBoard(1, 0); };
        if(i == 1) arrow.onclick = function() { moveBoard(-1, 0); };
        if(i == 2) arrow.onclick = function() { moveBoard(0, -1); };
        if(i == 3) arrow.onclick = function() { moveBoard(0, 1); };
        board.append(arrow);
    }
}

function getAlive() { return alive_cells; }

function randBoard() {
    let rand = Math.floor(Math.random() * (board_width*board_height));
    for(let i = 0; i < rand; i++) {
        setStateWithPos((Math.floor(Math.random() * (board_width-widthOffset))),(Math.floor(Math.random() * (board_height-widthOffset))),Math.ceil(Math.random() * 4));
    }
    if(activeStepInterval) startSteps();
}

function setStateWithPos(x, y, state) { // pass virtual coords
    updateAliveCellsForSetState([x+widthOffset, y+heightOffset].toString(), state);
    // console.log();
    space = getSpace(x, y);
    setAnimationsForSetState(space, state);
}

//takes a space == virtual coords
function setStateWithElement(space, state) {
    let coordOfStrs = space.id.split(',');
    updateAliveCellsForSetState([(parseInt(coordOfStrs[0],10))+widthOffset, (parseInt(coordOfStrs[1],10))+heightOffset].toString(), state);
    setAnimationsForSetState(space, state);
}

//takes in real coords
function updateAliveCellsForSetState(coordStr, state) {
    let coordOfStrs = coordStr.split(',');
    let coordStrMod = [(parseInt(coordOfStrs[0],10)), (parseInt(coordOfStrs[1],10))].toString();
    if(state == DEAD_STATE) {
        alive_cells.delete(coordStrMod);
    } else {
        alive_cells.set(coordStrMod,state);
    }
}
function setStateVisually(x, y, state, skipAnimation=false) { // pass virtuall coords
    let space = getSpace(x+widthOffset,y+heightOffset);
    setAnimationsForSetState(space,state,skipAnimation);
}
function setAnimationsForSetState(space, state, skipAnimation=false) {
    if(skipAnimation) {
        space.className = `space state-${state}`;
        return;
    }
    space.classList.add("shrink");
    setTimeout(() => {
        space.className = `space pop-out state-${state}`;
    }, 150);
}

//takes real coords
function getSpace(x, y) {
    return document.getElementById(`${x-widthOffset},${y-heightOffset}`);
}

function moveBoard(x, y) {
    subtractMap(alive_cells, true);
    widthOffset += x;
    heightOffset += y;
    addMap(alive_cells, true);
}

// void subtractMap(Map)
function subtractMap(map, skipAnimation=false) {
    map.forEach((value, coordStrOriginal) => {
        //coord[0] is x, coord[1] is y
        let coordOfStrs = coordStrOriginal.split(',');
        let coord = [parseInt(coordOfStrs[0],10), parseInt(coordOfStrs[1],10)];
        if(coord[0] >= widthOffset && coord[0] < board_width+widthOffset && coord[1] >= heightOffset && coord[1] < board_height+heightOffset) {
            setStateVisually(coord[0]-widthOffset,coord[1]-heightOffset, 0,skipAnimation);
        }
    });
}

// void addMap(Map)
function addMap(map, skipAnimation=false) {
    map.forEach((value, coordStrOriginal) => {
        //coord[0] is x, coord[1] is y
        let coordOfStrs = coordStrOriginal.split(',');
        let coord = [parseInt(coordOfStrs[0],10), parseInt(coordOfStrs[1],10)];
        if(coord[0] >= widthOffset && coord[0] < board_width+widthOffset && coord[1] >= heightOffset && coord[1] < board_height+heightOffset && value != getSpace(coord[0], coord[1]).className.split("state-")[1]) {
            setStateVisually(coord[0]-widthOffset,coord[1]-heightOffset, value,skipAnimation);
        }
    });
}

//call once to make a step
function doStep() {
    // console.log("Before:");
    // console.log(alive_cells);
    let new_alive_cells = step(alive_cells);
    let old_alive_cells = mapDifference(alive_cells, new_alive_cells);
    subtractMap(old_alive_cells);
    addMap(new_alive_cells);
    alive_cells = new_alive_cells;
    if(alive_cells.size === 0 && satisfyingMode) {
        stopSteps();
        setTimeout(() => { 
            randBoard();
            startSteps(); 
        }, speed);
    }
    scorePoints();
    updateScore();
}

//call after a step; score appropriate points
function scorePoints() {
    let i = 0;
    for(let cat of activeScoringCategories) {
        switch(cat.category) {
            case "cellBorn": 
                points += scoringCategories.cellBorn * cat.mult;
                break;
            case "cellDies":
                points += scoringCategories.cellDies * cat.mult;
                break;
            case "cellAlive":
                points += scoringCategories.cellAlive * cat.mult;
                break;
            case "maxYDist":
                points += scoringCategories.maxYDist * cat.mult;
                break;
            case "maxXDist":
                points += scoringCategories.maxXDist * cat.mult;
                break;
            case "colorBalance":
                points += scoringCategories.colorBalance * cat.mult;
                break;
            case "colorDominance":
                points += scoringCategories.colorDominance * cat.mult;
                break;
            default: break;
        }
    }
}
//call after a step; score appropriate points
function setupConditionsArray() {
    let i = 0;
    for(let cat of activeScoringCategories) {
        switch(cat.category) {
            case "cellBorn": 
                displayScoreConditions[i++] = `${cat.mult}x points for each cell born.`;
                break;
            case "cellDies":
                displayScoreConditions[i++] = `${cat.mult}x points for each cell that dies.`;
                break;
            case "cellAlive":
                displayScoreConditions[i++] = `${cat.mult}x points for each cell that stays alive.`;
                break;
            case "maxYDist":
                displayScoreConditions[i++] = `${cat.mult}x points for having great vertical distance between your cells.`;
                break;
            case "maxXDist":
                displayScoreConditions[i++] = `${cat.mult}x points for having great horizontal distance between your cells.`;
                break;
            case "colorBalance":
                displayScoreConditions[i++] = `${cat.mult}x multiplier for making your board look like a pack of skittles.`;
                break;
            case "colorDominance":
                displayScoreConditions[i++] = `${cat.mult}x multiplier for favoring one color above the others.`;
                break;
            default: break;
        }
    }
}

//call once between two steps to choose new scoring categories
function assignNewScoringCategories() {
    activeScoringCategories = [];

    let cat1 = Math.floor(Math.random() * 7);
    let cat2 = cat1; while(cat1 == cat2) { cat2 = Math.floor(Math.random() * 7); }
    let cat3 = cat1; while(cat3 == cat1 || cat3 == cat2) { cat3 = Math.floor(Math.random() * 7); }

    let range = allScoringCategoryMultiplyers[cat1];
    let mult = Math.floor(Math.random() * (range[1]-range[0]))+range[0];
    activeScoringCategories.push({category: allScoringCategories[cat1], mult: mult});
    
    range = allScoringCategoryMultiplyers[cat2];
    mult = Math.floor(Math.random() * (range[1]-range[0]))+range[0];
    activeScoringCategories.push({category: allScoringCategories[cat2], mult: mult});
    
    range = allScoringCategoryMultiplyers[cat3];
    mult = Math.floor(Math.random() * (range[1]-range[0]))+range[0];
    activeScoringCategories.push({category: allScoringCategories[cat3], mult: mult});

    setupConditionsArray();
    formatConditions();
    
}

function mapDifference(map1, map2) {
    const diff = new Map();
  
    // Check for keys in map1 that are not in map2
    for (const [key, value] of map1) {
        if (!map2.has(key) || map2.get(key) !== value) {
            diff.set(key, value);
        }
    }
  
    return diff;
}

function startSteps() {
    document.getElementById("goButton").style.display = "none";
    document.getElementById("stopButton").style.display = "flex";
    document.getElementById("stopButton").style.filter = "brightness(125%) hue-rotate(180deg)";

    arrows = document.getElementsByClassName("arrow");
    // console.log(arrows);
    for(let arrow of arrows) {
        arrow.style.filter = "opacity(0%)";
        setTimeout(() => {
            arrow.style.display = "none";
        }, 300);
    }

    clearInterval(stepInterval);
    stepInterval = setInterval(() => { doStep() }, speed);
    activeStepInterval = true;
}
function stopSteps() {
    document.getElementById("stopButton").style.display = "none";
    document.getElementById("goButton").style.display = "flex";
    document.getElementById("goButton").style.filter = "brightness(125%) hue-rotate(-45deg)";

    arrows = document.getElementsByClassName("arrow");
    // console.log(arrows);
    for(let arrow of arrows) {
        arrow.style.display = "flex";
        setTimeout(() => {
            arrow.style.filter = "opacity(50%)";
        }, 50);
    }

    clearInterval(stepInterval);
    activeStepInterval = false;
}

function toggleTurbo() {
    if(speed === 500) {
        speed = 100;
        document.getElementById("turboButton").style.filter = "brightness(125%) hue-rotate(-45deg)";
        if(activeStepInterval) startSteps();
    }
    else if(speed === 100) {
        speed = 50;
        document.getElementById("turboButton").style.filter = "brightness(125%) hue-rotate(-120deg)";
        if(activeStepInterval) startSteps();
    }
    else if(speed === 50) {
        speed = 10;
        document.getElementById("turboButton").style.filter = "brightness(125%) hue-rotate(-180deg)";
        if(activeStepInterval) startSteps();
    }
    else {
        speed = 500;
        document.getElementById("turboButton").style.filter = "brightness(100%) hue-rotate(0)";
        if(activeStepInterval) startSteps();
    }
}