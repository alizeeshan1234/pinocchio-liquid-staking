import * as anchor from '@coral-xyz/anchor';
import { describe, it } from 'mocha';
import { expect } from 'chai';
import { Connection, PublicKey, Keypair, SystemProgram, Transaction, TransactionInstruction } from '@solana/web3.js';
import { AnchorProvider, Wallet } from '@coral-xyz/anchor';
import fs from "fs";
import { createMint, TOKEN_PROGRAM_ID } from "@solana/spl-token";

const idl = JSON.parse(
    fs.readFileSync("./idl/staking_platform.json", "utf-8")
);

console.log("IDL metadata: ", idl.metadata);
console.log("IDL address: ", idl.metadata?.address);

describe('Staking Program Tests - Debug Version', function () {
    this.timeout(30000);

    let connection: Connection;
    let provider: AnchorProvider;
    let programId: PublicKey;

    let globalConfigAccountPda: PublicKey;
    let treasuryAccountPda: PublicKey;

    let mint: PublicKey;

    let stakingPoolPda: PublicKey;
    let stakeTokenVaultPda: PublicKey;
    let rewardTokenVaultPda: PublicKey;
    let liquidStakeMintPda: PublicKey;
    let rewardMint: PublicKey;

    const POOL_ID = 33;
    let payer: Keypair;   
    const poolIdBytes = Buffer.alloc(8);
    poolIdBytes.writeBigUInt64LE(BigInt(POOL_ID));

    before(async function () {
        connection = new Connection('https://api.devnet.solana.com', 'confirmed');

        try {
            const secretKey = JSON.parse(fs.readFileSync('./wallet.json', 'utf8'));
            payer = Keypair.fromSecretKey(Uint8Array.from(secretKey));
            console.log("Successfully loaded wallet from wallet.json");
            console.log("Wallet Public Key:", payer.publicKey.toString());
        } catch (error) {
            console.error("Error loading wallet from wallet.json:", error);
            console.log("Generating a new temporary Keypair instead.");
            payer = Keypair.generate();
            
            console.log("⚠️  You may need to airdrop SOL to this wallet for testing");
        }

        const wallet = new Wallet(payer);
        provider = new AnchorProvider(connection, wallet, { commitment: 'confirmed' });
        
        programId = new PublicKey(idl.metadata.address);

        const balance = await connection.getBalance(payer.publicKey);
        console.log("Wallet balance:", balance / anchor.web3.LAMPORTS_PER_SOL, "SOL");
        
        if (balance < 0.1 * anchor.web3.LAMPORTS_PER_SOL) {
            console.log("⚠️  Low wallet balance. You may need more SOL for testing.");
        }

        mint = await createMint(
            provider.connection,
            payer,
            payer.publicKey,
            null,
            6
        );

        rewardMint = await createMint(
            provider.connection,
            payer,
            provider.wallet.publicKey,
            null,
            6
        );

        [globalConfigAccountPda] = PublicKey.findProgramAddressSync(
            [Buffer.from("global_config_account"), provider.wallet.publicKey.toBuffer()],
            programId
        );

        [treasuryAccountPda] = PublicKey.findProgramAddressSync(
            [Buffer.from("treasury_account"), mint.toBuffer(), provider.wallet.publicKey.toBuffer()],
            programId
        );

        [stakingPoolPda] = PublicKey.findProgramAddressSync(
            [Buffer.from("staking_pool"), provider.wallet.publicKey.toBuffer(), poolIdBytes],
            programId
        );

        [stakeTokenVaultPda] = PublicKey.findProgramAddressSync(
            [Buffer.from("stake_token_vault"), mint.toBuffer(), globalConfigAccountPda.toBuffer()],
            programId
        );

        [rewardTokenVaultPda] = PublicKey.findProgramAddressSync(
            [Buffer.from("reward_token_vault"), rewardMint.toBuffer(), globalConfigAccountPda.toBuffer()],
            programId
        );

        [liquidStakeMintPda] = PublicKey.findProgramAddressSync(
            [Buffer.from("liquid_stake_mint"), provider.wallet.publicKey.toBuffer()],
            programId
        );

        console.log("Program ID:", programId.toString());
        console.log("Global Config PDA:", globalConfigAccountPda.toString());
        console.log("Treasury PDA:", treasuryAccountPda.toString());
        console.log("Mint:", mint.toString());
    });

    it("Initialize Global Config Account - Debug", async () => {
        console.log("\n=== Debug: Checking Program Account ===");
        
        const programAccount = await connection.getAccountInfo(programId);
        if (!programAccount) {
            console.error("Program not found!");
            return;
        }
        console.log("Program account found");
        console.log("Program owner:", programAccount.owner.toString());
        console.log("Program executable:", programAccount.executable);
        
        const globalConfigInfo = await connection.getAccountInfo(globalConfigAccountPda);
        const treasuryInfo = await connection.getAccountInfo(treasuryAccountPda);

        console.log("Global Config exists:", !!globalConfigInfo);
        console.log("Treasury exists:", !!treasuryInfo);

        if (globalConfigInfo || treasuryInfo) {
            console.log("One or both accounts already exist! Skipping test.");
            return;
        }

        try {
            const protocolFeeRate = 500;
            const minStakeAmount = 1000000;
            const maxPools = 100;

            const protocolFeeRateBuffer = Buffer.alloc(2);
            protocolFeeRateBuffer.writeUInt16LE(protocolFeeRate);

            const minStakeAmountBuffer = Buffer.alloc(8);
            minStakeAmountBuffer.writeBigUInt64LE(BigInt(minStakeAmount));

            const maxPoolBuffer = Buffer.alloc(4);
            maxPoolBuffer.writeUInt32LE(maxPools);

            const instructionData = Buffer.concat([
                protocolFeeRateBuffer,   // 0-1: protocol_fee_rate (u16)
                minStakeAmountBuffer,    // 2-9: min_stake_amount (u64)  
                maxPoolBuffer            // 10-13: max_pools (u32)
            ]);

            console.log("=== Instruction Data Breakdown ===");
            console.log("Protocol Fee Rate:", protocolFeeRate);
            console.log("Protocol Fee Rate Buffer:", protocolFeeRateBuffer.toString('hex'));
            console.log("Min Stake Amount:", minStakeAmount);
            console.log("Min Stake Amount Buffer:", minStakeAmountBuffer.toString('hex'));
            console.log("Max Pools:", maxPools);
            console.log("Max Pool Buffer:", maxPoolBuffer.toString('hex'));
            console.log("Combined instruction data length:", instructionData.length);
            console.log("Combined instruction data (hex):", instructionData.toString('hex'));

            // Final data with discriminator
            const finalInstructionData = Buffer.concat([
                Buffer.from([0]), // discriminator for InitConfigAccount
                instructionData   // actual instruction data (14 bytes)
            ]);

            console.log("Final data with discriminator length:", finalInstructionData.length);
            console.log("Final data with discriminator (hex):", finalInstructionData.toString('hex'));

            // Verify the data parsing matches what Rust expects
            console.log("\n=== Verification of Data Parsing ===");
            const discriminator = finalInstructionData[0];
            const dataAfterDisc = finalInstructionData.slice(1);
            console.log("Discriminator:", discriminator);
            console.log("Data after discriminator length:", dataAfterDisc.length);
            console.log("Data after discriminator (hex):", dataAfterDisc.toString('hex'));

            // Parse back to verify
            const parsedProtocolFeeRate = dataAfterDisc.readUInt16LE(0);
            const parsedMinStakeAmount = dataAfterDisc.readBigUInt64LE(2);
            const parsedMaxPools = dataAfterDisc.readUInt32LE(10);

            console.log("Parsed Protocol Fee Rate:", parsedProtocolFeeRate);
            console.log("Parsed Min Stake Amount:", parsedMinStakeAmount.toString());
            console.log("Parsed Max Pools:", parsedMaxPools);

            console.log("\n=== Account Keys ===");
            console.log("Authority (signer):", provider.wallet.publicKey.toString());
            console.log("Mint:", mint.toString());
            console.log("Global Config PDA:", globalConfigAccountPda.toString());
            console.log("Treasury PDA:", treasuryAccountPda.toString());
            console.log("System Program:", SystemProgram.programId.toString());
            console.log("Token Program:", TOKEN_PROGRAM_ID.toString());

            // Create the transaction instruction
            const instruction = new TransactionInstruction({
                programId: programId,
                keys: [
                    { pubkey: provider.wallet.publicKey, isSigner: true, isWritable: true },     // authority
                    { pubkey: mint, isSigner: false, isWritable: false },                        // mint
                    { pubkey: globalConfigAccountPda, isSigner: false, isWritable: true },       // global_config_account
                    { pubkey: treasuryAccountPda, isSigner: false, isWritable: true },           // treasury_account
                    { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },     // system_program
                    { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },            // token_program
                ],
                data: finalInstructionData
            });

            console.log("\n=== Sending Transaction ===");
            const transaction = new Transaction().add(instruction);
            
            const { blockhash } = await connection.getLatestBlockhash();
            transaction.recentBlockhash = blockhash;
            transaction.feePayer = provider.wallet.publicKey;

            // Try to simulate first
            console.log("Simulating transaction...");
            try {
                const simulationResult = await connection.simulateTransaction(transaction);
                console.log("Simulation result:", simulationResult);
            } catch (simError: any) {
                console.error("Simulation failed:", simError);
                console.error("Simulation logs:", simError.logs);
            }

            const sig = await provider.sendAndConfirm(transaction, []);
            console.log("✅ Transaction Signature:", sig);
            
            // Verify accounts were created
            const newGlobalConfigInfo = await connection.getAccountInfo(globalConfigAccountPda);
            const newTreasuryInfo = await connection.getAccountInfo(treasuryAccountPda);
            
            console.log("\n=== Post-transaction Verification ===");
            console.log("Global Config created:", !!newGlobalConfigInfo);
            console.log("Treasury created:", !!newTreasuryInfo);
            
            if (newGlobalConfigInfo) {
                console.log("Global Config owner:", newGlobalConfigInfo.owner.toString());
                console.log("Global Config data length:", newGlobalConfigInfo.data.length);
            }
            
            if (newTreasuryInfo) {
                console.log("Treasury owner:", newTreasuryInfo.owner.toString());
                console.log("Treasury data length:", newTreasuryInfo.data.length);
            }
            
        } catch (error: any) {
            console.error("\n❌ Transaction failed:", error.message);
            
            if (error.logs) {
                console.error("\n📋 Transaction Logs:");
                error.logs.forEach((log: string, index: number) => {
                    console.error(`${index}: ${log}`);
                });
            }

            // Additional debugging based on the error
            console.error("\n🔍 Potential Issues:");
            console.error("1. Program might not be deployed or wrong program ID");
            console.error("2. Instruction data format mismatch");
            console.error("3. Account order mismatch between TS and Rust");
            console.error("4. PDA calculation mismatch");
            console.error("5. Missing required accounts or wrong account types");
            
            throw error;
        }
    });

    it("Process Update Authority", async () => {
        let newAuthority = provider.wallet.publicKey;
        let instructionData = Buffer.concat([
            newAuthority.toBuffer()
        ]);

        const finalInstructionData = Buffer.concat([
            Buffer.from([1]),
            instructionData
        ]);

        const instruction = new TransactionInstruction({
            programId: programId,
            keys: [
                { pubkey: provider.wallet.publicKey, isSigner: true, isWritable: true },     // authority
                { pubkey: globalConfigAccountPda, isSigner: false, isWritable: true },       // global_config_account
            ],
            data: finalInstructionData
        });

        const transaction = new Transaction().add(instruction);

        const { blockhash } = await connection.getLatestBlockhash();
        transaction.recentBlockhash = blockhash;
        transaction.feePayer = provider.wallet.publicKey;

        console.log("Simulating transaction...");
        try {
            const simulationResult = await connection.simulateTransaction(transaction);
            console.log("Simulation result:", simulationResult);
        } catch (simError: any) {
            console.error("Simulation failed:", simError);
            console.error("Simulation logs:", simError.logs);
        }

        const sig = await provider.sendAndConfirm(transaction, []);
        console.log("✅ Transaction Signature:", sig); 
    });

    it("Process Update Protocol Fees", async () => {
        const protocolFeeRate = 3000;
        const protocolFeeRateBuffer = Buffer.alloc(2);
        protocolFeeRateBuffer.writeUInt16LE(protocolFeeRate);

        const instructionData = Buffer.concat([
            protocolFeeRateBuffer
        ]);

        const finalInstructionData = Buffer.concat([
            Buffer.from([2]),
            instructionData
        ]);

        const instruction = new TransactionInstruction({
            programId: programId,
            keys: [
                { pubkey: provider.wallet.publicKey, isSigner: true, isWritable: true },     // authority
                { pubkey: globalConfigAccountPda, isSigner: false, isWritable: true },       // global_config_account
            ],
            data: finalInstructionData
        });

        const transaction = new Transaction().add(instruction);

        const { blockhash } = await connection.getLatestBlockhash();
        transaction.recentBlockhash = blockhash;
        transaction.feePayer = provider.wallet.publicKey;

        console.log("Simulating transaction...");
        try {
            const simulationResult = await connection.simulateTransaction(transaction);
            console.log("Simulation result:", simulationResult);
        } catch (simError: any) {
            console.error("Simulation failed:", simError);
            console.error("Simulation logs:", simError.logs);
        }

        const sig = await provider.sendAndConfirm(transaction, []);
        console.log("✅ Transaction Signature:", sig); 
    });

    it("Create Staking Pool Account", async () => {
        // Fixed values matching your current code
        const REWARD_RATE_PER_SECOND = 100;
        const LOCK_PERIOD_ENABLED = 1;  // 1 = true, 0 = false
        const LOCK_PERIOD_DURATION = 100000;
        const REWARD_MULTIPLIER = 150;  // Must be > 0 per your validation
        const EARLY_WITHDRAW_PENALTY = 500;
        const SLASHING_ENABLED = 1;  // 1 = true, 0 = false
        const SLASHING_CONDITION_TYPE = 1;  // Must be valid SlashTypeEnum
        const SLASH_PERCENTAGE = 1000;  // In basis points (10%)
        const MIN_EVIDENCE_REQUIRED = 5;
        const COOLDOWN_PERIOD = 100000;
        const MAXIMUM_STAKE_LIMIT = 50000;  // Fixed variable name
        const MINIMUM_STAKE_AMOUNT = 10000;

        console.log("Creating staking pool with ID:", POOL_ID);

        // Fix PDA generation - pool_id needs to be u64 bytes, not single byte
        const poolIdBytes = Buffer.alloc(8);
        poolIdBytes.writeBigUInt64LE(BigInt(POOL_ID));
    

        // Build instruction data - exactly 64 bytes
        const poolIdBuffer = Buffer.alloc(8);
        poolIdBuffer.writeBigUInt64LE(BigInt(POOL_ID));

        const rewardRateBuffer = Buffer.alloc(8);
        rewardRateBuffer.writeBigUInt64LE(BigInt(REWARD_RATE_PER_SECOND));

        const lockPeriodEnabledBuffer = Buffer.from([LOCK_PERIOD_ENABLED]);

        const lockPeriodDurationBuffer = Buffer.alloc(8);
        lockPeriodDurationBuffer.writeBigInt64LE(BigInt(LOCK_PERIOD_DURATION));

        const rewardMultiplierBuffer = Buffer.alloc(2);
        rewardMultiplierBuffer.writeUInt16LE(REWARD_MULTIPLIER);

        const earlyWithdrawPenaltyBuffer = Buffer.alloc(8);
        earlyWithdrawPenaltyBuffer.writeBigUInt64LE(BigInt(EARLY_WITHDRAW_PENALTY));

        const slashingEnabledBuffer = Buffer.from([SLASHING_ENABLED]);

        const slashingConditionTypeBuffer = Buffer.from([SLASHING_CONDITION_TYPE]);

        const slashPercentageBuffer = Buffer.alloc(2);
        slashPercentageBuffer.writeUInt16LE(SLASH_PERCENTAGE);

        const minEvidenceRequiredBuffer = Buffer.from([MIN_EVIDENCE_REQUIRED]);

        const cooldownPeriodBuffer = Buffer.alloc(8);
        cooldownPeriodBuffer.writeBigInt64LE(BigInt(COOLDOWN_PERIOD));

        const maximumStakeLimitBuffer = Buffer.alloc(8);
        maximumStakeLimitBuffer.writeBigUInt64LE(BigInt(MAXIMUM_STAKE_LIMIT));

        const minimumStakeAmountBuffer = Buffer.alloc(8);
        minimumStakeAmountBuffer.writeBigUInt64LE(BigInt(MINIMUM_STAKE_AMOUNT));

        const instructionData = Buffer.concat([
            poolIdBuffer,                    // 0-7: pool_id (u64)
            rewardRateBuffer,               // 8-15: reward_rate_per_second (u64)
            lockPeriodEnabledBuffer,        // 16: lock_period_enabled (u8)
            lockPeriodDurationBuffer,       // 17-24: lock_period_duration (i64)
            rewardMultiplierBuffer,         // 25-26: reward_multiplier (u16)
            earlyWithdrawPenaltyBuffer,     // 27-34: early_withdraw_penalty (u64)
            slashingEnabledBuffer,          // 35: slashing_enabled (u8)
            slashingConditionTypeBuffer,    // 36: slashing_condition_type (u8)
            slashPercentageBuffer,          // 37-38: slash_percentage (u16)
            minEvidenceRequiredBuffer,      // 39: min_evidence_required (u8)
            cooldownPeriodBuffer,           // 40-47: cooldown_period (i64)
            maximumStakeLimitBuffer,        // 48-55: maximum_stake_limit (u64)
            minimumStakeAmountBuffer        // 56-63: minimum_stake_amount (u64)
        ]);

        console.log("Instruction data length:", instructionData.length, "(should be 64)");

        const finalInstructionData = Buffer.concat([
            Buffer.from([3]), // discriminator for CreateStakingPool
            instructionData
        ]);

        const priceFeedAccount = Keypair.generate().publicKey;

        const instruction = new TransactionInstruction({
            programId: programId,
            keys: [
                { pubkey: provider.wallet.publicKey, isSigner: true, isWritable: true },    // authority
                { pubkey: provider.wallet.publicKey, isSigner: true, isWritable: true },      // creator
                { pubkey: mint, isSigner: false, isWritable: false },                         // stake_token_mint
                { pubkey: rewardMint, isSigner: false, isWritable: false },                   // reward_token_mint
                { pubkey: stakeTokenVaultPda, isSigner: false, isWritable: true },            // stake_token_vault
                { pubkey: rewardTokenVaultPda, isSigner: false, isWritable: true },           // reward_token_vault
                { pubkey: stakingPoolPda, isSigner: false, isWritable: true },                // staking_pool_account
                { pubkey: globalConfigAccountPda, isSigner: false, isWritable: true },        // global_config_account
                { pubkey: liquidStakeMintPda, isSigner: false, isWritable: true },            // liquid_stake_mint
                { pubkey: priceFeedAccount, isSigner: false, isWritable: false },             // price_feed_account
                { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },      // system_program
                { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },             // token_program
            ],
            data: finalInstructionData
        });

        console.log("\n=== Account Keys ===");
        console.log("Authority:", provider.wallet.publicKey.toString());
        console.log("Creator:", provider.wallet.publicKey.toString());
        console.log("Stake Token Mint:", mint.toString());
        console.log("Reward Token Mint:", rewardMint.toString());
        console.log("Stake Token Vault PDA:", stakeTokenVaultPda.toString());
        console.log("Reward Token Vault PDA:", rewardTokenVaultPda.toString());
        console.log("Staking Pool PDA:", stakingPoolPda.toString());
        console.log("Global Config PDA:", globalConfigAccountPda.toString());
        console.log("Liquid Stake Mint PDA:", liquidStakeMintPda.toString());

        const transaction = new Transaction().add(instruction);

        const { blockhash } = await connection.getLatestBlockhash();
        transaction.recentBlockhash = blockhash;
        transaction.feePayer = provider.wallet.publicKey;

        console.log("Simulating transaction...");
        try {
            const simulationResult = await connection.simulateTransaction(transaction);
            console.log("Simulation result:", simulationResult);
        } catch (simError: any) {
            console.error("Simulation failed:", simError);
            console.error("Simulation logs:", simError.logs);
        }

        const sig = await provider.sendAndConfirm(transaction, []);
        console.log("✅ Transaction Signature:", sig);

        // Verify the staking pool was created
        const stakingPoolInfo = await connection.getAccountInfo(stakingPoolPda);
        console.log("Staking Pool created:", !!stakingPoolInfo);
        if (stakingPoolInfo) {
            console.log("Staking Pool owner:", stakingPoolInfo.owner.toString());
            console.log("Staking Pool data length:", stakingPoolInfo.data.length);
        }
    });
});