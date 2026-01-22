import { EventEmitter } from 'events';
import { ethers } from 'ethers';
import WebSocket from 'ws';
import Redis from 'ioredis';
import { LRUCache } from 'lru-cache';
import { createPublicClient, http, parseAbiItem } from 'viem';
import { mainnet } from 'viem/chains';
import Big from 'big.js';
import Decimal from 'decimal.js-light';
import JSBI from 'jsbi';
import invariant from 'tiny-invariant';
import { backOff } from 'exponential-backoff';
import { Command } from 'commander';
import 'dotenv/config';

console.log("All imports work!");
