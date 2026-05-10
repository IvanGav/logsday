const NO_STATE_CHANGE = -1;
const DEAD_STATE = 0;
const PROTECTED = 0.4;
let devmode = false;


function switchdev() {
    devmode = !devmode;
}

let currentRules = [ // conways by default
    {type: 0, numCells: 2, comparison: 0, neighborState: 1},
    {type: 1, numCells: 2, comparison: 2, neighborState: 1},
    {type: 1, numCells: 3, comparison: 2, neighborState: 1},
    {type: 0, numCells: 3, comparison: 4, neighborState: 1},
    {type: 2, numCells: 3, comparison: 2, neighborState: 1, babyState: 1},
];

let evolvOrBorn = [true, false, false, false];

function resetRules() {
    currentRules = [ // conways by default
        {type: 0, numCells: 2, comparison: 0, neighborState: 1},
        {type: 1, numCells: 2, comparison: 2, neighborState: 1},
        {type: 1, numCells: 3, comparison: 2, neighborState: 1},
        {type: 0, numCells: 3, comparison: 4, neighborState: 1},
        {type: 2, numCells: 3, comparison: 2, neighborState: 1, babyState: 1},
    ];
    // putUpRules();
}

function importRules(ruleStr) {
    currentRules = JSON.parse(ruleStr);
    putUpRules();
}

function exportRules() {
    return JSON.stringify(currentRules);
}

function putUpRules() {
    let rulesContainer = document.getElementById("rules");
    rulesContainer.innerHTML = "";
    let rulesTitle = document.createElement("h1");
    rulesTitle.innerHTML = "Rules";
    rules.append(rulesTitle);
    for(let i = 0; i < currentRules.length; i++) {
        let content = document.createTextNode(`${i+1}. ` + ruleParser(currentRules[i]));
        rulesContainer.append(content);
        rulesContainer.append(document.createElement("br"));
        rulesContainer.append(document.createElement("br"));
    }
    
    // addRandRule();
}

function resetEvolOrBorn() {
    evolvOrBorn = [true, false, false, false];
}

function genRandRule() {
    // console.log(`rangenrule`);
    let type = Math.floor(Math.random()*4);
    let comparison = Math.floor(Math.random()*5);
    let numCells = Math.floor(Math.random()*7)+1;
    
    let neighborState = Math.ceil(Math.random()*4);
    
    while(!evolvOrBorn[neighborState-1]) {
        neighborState = Math.ceil(Math.random()*4);
    }
    // if (neighborState != 1) {
    //     console.log(`THIS SHOULD NOT BE POSSIBLE RAH: ${neighborState} ${evolvOrBorn}`);
    // }
    
    let babyState = Math.ceil(Math.random()*4);
    let myState = Math.ceil(Math.random()*4);
    while(!evolvOrBorn[myState-1]) {
        myState = Math.ceil(Math.random()*4);
    }
    
    while (myState == babyState) {
        babyState = Math.ceil(Math.random()*4);
    }
    if (type == 2 || type == 3) {
        evolvOrBorn[babyState-1] = true;
    }

    //make sure that you can't be born with < or <= conditions
    if (type == 2 && comparison < 2) {
        comparison = Math.floor(Math.random()*3)+2;
    }

    // console.log(`type = ${type}\ncomparison = ${comparison}\nnumcells = ${numCells}\nneighborState = ${neighborState}\nbabystate = ${babyState}\nmystate = ${myState}`);
    switch(type) {
        case 0:
        case 1:
            return ({
                type: type,
                comparison: comparison,
                numCells: numCells,
                neighborState: neighborState,
            });
        case 2:
            return ({
                type: type,
                comparison: comparison,
                numCells: numCells,
                neighborState: neighborState,
                babyState: babyState,
            });
        case 3:
            return ({
                type: type,
                comparison: comparison,
                numCells: numCells,
                neighborState: neighborState,
                myState: myState,
                babyState: babyState,
            });
    }
}

function addRule(newRule) {
    currentRules.append(newRule);
}

function addRandRule() {
    currentRules.push(genRandRule());

    let rulesContainer = document.getElementById("rules");
    let content = document.createTextNode(`${currentRules.length}. ` + ruleParser(currentRules[currentRules.length - 1]));
    rulesContainer.append(content);
    rulesContainer.append(document.createElement("br"));
    rulesContainer.append(document.createElement("br"));
}

let allScoringCategories = [
    "cellBorn",
    "cellDies",
    "cellAlive",
    "maxYDist",
    "maxXDist",
    "colorBalance",
    "colorDominance"
];

let allScoringCategoryMultiplyers = [
    [-1, 7],
    [-5, 5],
    [1, 3],
    [-2, 8],
    [-2, 8],
    [-3, 3],
    [2, 8]
];

let scoringCategories = {
    "cellBorn": 0,
    "cellDies": 0,
    "cellAlive": 0,
    "maxYDist": 0,
    "maxXDist": 0,
    "colorBalance": 0,
    "colorDominance": 0
};

function resetScores() {
    scoringCategories = {
        "cellBorn": 0,
        "cellDies": 0,
        "cellAlive": 0,
        "maxYDist": 0,
        "maxXDist": 0,
        "colorBalance": 0,
        "colorDominance": 0
    };
}

// Any variables with `Map` at the end, are maps.
// Any variables with `Coord` at the end, are [x,y] arrays.
// Any variables with `State` at the end, are numbers, representing the states of a cell.

// applyRules should be a function pointer such that:
// returns: number checkAtNewState
// params:  Map curStateMap, [number, number] checkAtCoord
let applyRules = function(curStateMap, checkAtCoord) {
    let max = NO_STATE_CHANGE;
    for(let rule of currentRules) {
        let result = max;
        switch (rule.type) {
            case 0:
                result = generalDead(rule.numCells, rule.comparison, rule.neighborState, curStateMap, checkAtCoord);
                break;
            case 1:
                result = generalStay(rule.numCells, rule.comparison, rule.neighborState, curStateMap, checkAtCoord);
                break;
            case 2:
                result = generalRepr(rule.numCells, rule.comparison, rule.neighborState, rule.babyState, curStateMap, checkAtCoord);
                break;
            case 3:
                result = generalEvol(rule.numCells, rule.comparison, rule.neighborState, rule.myState, rule.babyState, curStateMap, checkAtCoord);
                break;
        }

        max = (max > result) ? max : result;
    }
    return max;
};

let generalDead = function(numCells, comparison, neighborState, curStateMap, checkAtCoord) {
    if (curStateMap.has(checkAtCoord.toString())) {
        let sum = sumNeighbor(curStateMap, checkAtCoord, neighborState);
        switch (comparison) {
            case 0:
                return (sum < numCells) ? DEAD_STATE : NO_STATE_CHANGE;
            case 1:
                return (sum <= numCells) ? DEAD_STATE : NO_STATE_CHANGE;
            case 2:
                return (sum == numCells) ? DEAD_STATE : NO_STATE_CHANGE;
            case 3:
                return (sum >= numCells) ? DEAD_STATE : NO_STATE_CHANGE;
            case 4:
                return (sum > numCells) ? DEAD_STATE : NO_STATE_CHANGE;
            }
    }
    return DEAD_STATE;
}

let generalStay = function(numCells, comparison, neighborState, curStateMap, checkAtCoord) {
    if (curStateMap.has(checkAtCoord.toString())) {
        let sum = sumNeighbor(curStateMap, checkAtCoord, neighborState);
        // console.log(sum);
        switch (comparison) {
            case 0:
                return (sum < numCells) ? PROTECTED : DEAD_STATE;
            case 1:
                return (sum <= numCells) ? PROTECTED : DEAD_STATE;
            case 2:
                return (sum == numCells) ? PROTECTED : DEAD_STATE;
            case 3:
                return (sum >= numCells) ? PROTECTED : DEAD_STATE;
            case 4:
                return (sum > numCells) ? PROTECTED : DEAD_STATE;
        }
    }
    return DEAD_STATE;
}

let generalRepr = function(numCells, comparison, neighborState, babyState, curStateMap, checkAtCoord) {
    if (!curStateMap.has(checkAtCoord.toString())) {
        let sum = sumNeighbor(curStateMap, checkAtCoord, neighborState);
        switch (comparison) {
            case 0:
                return (sum < numCells) ? babyState : DEAD_STATE;
            case 1:
                return (sum <= numCells) ? babyState : DEAD_STATE;
            case 2:
                return (sum == numCells) ? babyState : DEAD_STATE;
            case 3:
                return (sum >= numCells) ? babyState : DEAD_STATE;
            case 4:
                return (sum > numCells) ? babyState : DEAD_STATE;
        }
    }
    return NO_STATE_CHANGE;
}

let generalEvol = function(numCells, comparison, neighborState, myState, babyState, curStateMap, checkAtCoord) {
    if (curStateMap.has(checkAtCoord.toString()) && curStateMap.get(checkAtCoord.toString()) == myState) {
        let sum = sumNeighbor(curStateMap, checkAtCoord, neighborState);
        switch (comparison) {
            case 0:
                return (sum < numCells) ? babyState : NO_STATE_CHANGE;
            case 1:
                return (sum <= numCells) ? babyState : NO_STATE_CHANGE;
            case 2:
                return (sum == numCells) ? babyState : NO_STATE_CHANGE;
            case 3:
                return (sum >= numCells) ? babyState : NO_STATE_CHANGE;
            case 4:
                return (sum > numCells) ? babyState : NO_STATE_CHANGE;
        }
    }
    return NO_STATE_CHANGE;
}


// number sumNeighbor(Map, [number, number], number)
function sumNeighbor(curStateMap, checkAtCoord, value) {
    let sum = 
    (curStateMap.has([checkAtCoord[0]-1, checkAtCoord[1]-1].toString()) && curStateMap.get([checkAtCoord[0]-1, checkAtCoord[1]-1].toString()) == value) +
    (curStateMap.has([checkAtCoord[0], checkAtCoord[1]-1].toString()) && curStateMap.get([checkAtCoord[0], checkAtCoord[1]-1].toString()) == value) +
    (curStateMap.has([checkAtCoord[0]+1, checkAtCoord[1]-1].toString()) && curStateMap.get([checkAtCoord[0]+1, checkAtCoord[1]-1].toString()) == value) +
    (curStateMap.has([checkAtCoord[0]-1, checkAtCoord[1]].toString()) && curStateMap.get([checkAtCoord[0]-1, checkAtCoord[1]].toString()) == value) +
    (curStateMap.has([checkAtCoord[0]+1, checkAtCoord[1]].toString()) && curStateMap.get([checkAtCoord[0]+1, checkAtCoord[1]].toString()) == value) +
    (curStateMap.has([checkAtCoord[0]-1, checkAtCoord[1]+1].toString()) && curStateMap.get([checkAtCoord[0]-1, checkAtCoord[1]+1].toString()) == value) +
    (curStateMap.has([checkAtCoord[0], checkAtCoord[1]+1].toString()) && curStateMap.get([checkAtCoord[0], checkAtCoord[1]+1].toString()) == value) +
    (curStateMap.has([checkAtCoord[0]+1, checkAtCoord[1]+1].toString()) && curStateMap.get([checkAtCoord[0]+1, checkAtCoord[1]+1].toString()) == value);
    // let above = curStateMap.has([checkAtCoord[0], checkAtCoord[1]-1].toString());
    // console.log(`for coord: (${checkAtCoord}), sum: ${sum}, above: ${[checkAtCoord[0], checkAtCoord[1]-1].toString()} ${above}`);
    return sum;
}

//these 4 are for score calculations, no touching pls
let leftMost;
let rightMost;
let upMost;
let downMost;
//this is also for score calculations, no touching too pls
let cols;

//Map step(Map)
// can access scoringCategories after calling to step
function step(prevStateMap) {
    resetScores();
    leftMost = NaN; rightMost = NaN; upMost = NaN; downMost = NaN;
    cols = [0,0,0,0];
    let nextStateMap = new Map();
    let nextEmptyMap = new Map();
    prevStateMap.forEach((cellState, coordStrOriginal) => {
        let coordOfStrs = coordStrOriginal.split(',');
        let coord = [parseInt(coordOfStrs[0],10), parseInt(coordOfStrs[1],10)];
        let stateChange = false;
        //we could've already looked at it from its alive neighbors
        let cstr = coord.toString();
        // if(!nextStateMap.has(cstr) && !nextEmptyMap.has(cstr)) {
        let newState = applyRules(prevStateMap, coord);
        stateChange = prevStateMap.get(cstr) != newState;
        updateCoord(cstr, coord, prevStateMap, nextStateMap, nextEmptyMap);
        // }
        //if the cell didn't change, we don't need to update the neighbors
        if(stateChange) {
            //look at neighbors
            let c = [coord[0]-1, coord[1]-1];
            let cstr = c.toString();
            if(!prevStateMap.has(cstr)) updateCoord(cstr, c, prevStateMap, nextStateMap, nextEmptyMap);
            c = [coord[0], coord[1]-1];
            cstr = c.toString();
            if(!prevStateMap.has(cstr)) updateCoord(cstr, c, prevStateMap, nextStateMap, nextEmptyMap);
            c = [coord[0]+1, coord[1]-1];
            cstr = c.toString();
            if(!prevStateMap.has(cstr)) updateCoord(cstr, c, prevStateMap, nextStateMap, nextEmptyMap);

            c = [coord[0]-1, coord[1]];
            cstr = c.toString();
            if(!prevStateMap.has(cstr)) updateCoord(cstr, c, prevStateMap, nextStateMap, nextEmptyMap);
            c = [coord[0]+1, coord[1]];
            cstr = c.toString();
            if(!prevStateMap.has(cstr)) updateCoord(cstr, c, prevStateMap, nextStateMap, nextEmptyMap);

            c = [coord[0]-1, coord[1]+1];
            cstr = c.toString();
            if(!prevStateMap.has(cstr)) updateCoord(cstr, c, prevStateMap, nextStateMap, nextEmptyMap);
            c = [coord[0], coord[1]+1];
            cstr = c.toString();
            if(!prevStateMap.has(cstr)) updateCoord(cstr, c, prevStateMap, nextStateMap, nextEmptyMap);
            c = [coord[0]+1, coord[1]+1];
            cstr = c.toString();
            if(!prevStateMap.has(cstr)) updateCoord(cstr, c, prevStateMap, nextStateMap, nextEmptyMap);
        }
    });
    let colSum = cols[0]+cols[1]+cols[2]+cols[3];
    let colMax = Math.max(...cols);
    if(colSum == 0) {
        scoringCategories.maxXDist = 0;
        scoringCategories.maxYDist = 0;
        scoringCategories.colorDominance = 0;
        scoringCategories.colorBalance = 0;
    } else {
        //finish calulating stats
        scoringCategories.maxXDist = rightMost - leftMost;
        scoringCategories.maxYDist = upMost - downMost;
        scoringCategories.colorDominance = ((colMax / colSum) * 100) * (colSum/500); //percentage of most abundant color (scaled by # cells)
        scoringCategories.colorBalance = 0-((colMax - (colSum/4)) - colSum); //i wish i knew what it meant
    }
    return nextStateMap;
}

// void updateCoord(string coordStr, [number,number] coord, Map prevStateMap, Map nextStateMap, Map nextEmptyMap)
function updateCoord(coordStr, coord, prevStateMap, nextStateMap, nextEmptyMap) {
    if(!nextStateMap.has(coordStr) && !nextEmptyMap.has(coordStr)) {
        let oldState = prevStateMap.get(coordStr); 
        if(oldState == undefined) oldState = DEAD_STATE;
        
        let newState = applyRules(prevStateMap, coord);
        if(newState == PROTECTED || newState == NO_STATE_CHANGE) newState = oldState;
        
        if(newState == DEAD_STATE) {
            nextEmptyMap.set(coordStr, DEAD_STATE);
        } else {
            nextStateMap.set(coordStr, newState);
        }
        // score
        if(oldState == DEAD_STATE && newState != DEAD_STATE) {
            scoringCategories.cellBorn += 1;
        } else if(oldState != DEAD_STATE && newState == DEAD_STATE) {
            scoringCategories.cellDies += 1;
        } else if(oldState != DEAD_STATE && newState != DEAD_STATE) {
            scoringCategories.cellAlive += 1;
        }
        if(newState != DEAD_STATE) {
            leftMost = !(coord[0] > leftMost) ? coord[0] : leftMost;
            rightMost = !(coord[0] < rightMost) ? coord[0] : rightMost;
            downMost = !(coord[1] > downMost) ? coord[1] : downMost;
            upMost = !(coord[1] < upMost) ? coord[1] : upMost;
            cols[newState-1] += 1;
        }
    }
}

function ruleParser(rule) {
    str = "A cell will ";

    if(rule.type == 0) str += "die ";
    else if(rule.type == 1) str += "stay alive ";
    else if(rule.type == 2) {
        str += "be born as a ";
        if(rule.babyState == 1) str += "yellow ";
        else if(rule.babyState == 2) str += "pink ";
        else if(rule.babyState == 3) str += "green ";
        else if(rule.babyState == 4) str += "blue ";
        str += "cell ";
    }
    else if(rule.type == 3) {
        str += "evolve from a ";
        if(rule.myState == 1) str += "yellow ";
        else if(rule.myState == 2) str += "pink ";
        else if(rule.myState == 3) str += "green ";
        else if(rule.myState == 4) str += "blue ";
        str += "cell into a ";
        if(rule.babyState == 1) str += "yellow ";
        else if(rule.babyState == 2) str += "pink ";
        else if(rule.babyState == 3) str += "green ";
        else if(rule.babyState == 4) str += "blue ";
        str += "cell ";
    }
    
    str += "if it neighbors "

    if(rule.comparison == 0) str += "less than ";
    else if(rule.comparison == 1) str += "at most ";
    else if(rule.comparison == 2) str += "exactly ";
    else if(rule.comparison == 3) str += "at least ";
    else if(rule.comparison == 4) str += "more than ";
    
    str += rule.numCells + " ";

    if(rule.neighborState == 1) str += "yellow ";
    else if(rule.neighborState == 2) str += "pink ";
    else if(rule.neighborState == 3) str += "green ";
    else if(rule.neighborState == 4) str += "blue ";

    str += "cells.";

    return str;
}

function updateScore() {
    let scoreContainer = document.getElementById("scoreHeaderDiv");
    let turnHeader = scoreContainer.children[0];
    turnHeader.innerHTML = `Turn: ${Math.round(currentTurn++)}`;   
    let scoreHeader = scoreContainer.children[1];
    scoreHeader.innerHTML = `Score: ${Math.round(points)}`;    
    if(currentTurn > 100 && !devmode) {
        stopSteps();
        document.getElementById("board").remove();
        document.getElementById("genButton").style.display = "flex";
        document.getElementById("stopButton").style.display = "none";
        document.getElementById("goButton").style.display = "none";
        document.getElementById("stepButton").style.display = "none";
        document.getElementById("turboButton").style.display = "none";
        document.getElementById("randButton").style.display = "none";
    }   
}
function formatConditions() {
    let scorePre = document.getElementById("scorePre");
    scorePre.innerHTML = `${displayScoreConditions[0]}\n${displayScoreConditions[1]}\n${displayScoreConditions[2]}`;
}