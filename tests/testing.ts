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

    before(async function () {
        connection = new Connection('https://api.devnet.solana.com', 'confirmed');

        let payer: Keypair;
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

        [globalConfigAccountPda] = PublicKey.findProgramAddressSync(
            [Buffer.from("global_config_account"), provider.wallet.publicKey.toBuffer()],
            programId
        );

        [treasuryAccountPda] = PublicKey.findProgramAddressSync(
            [Buffer.from("treasury_account"), mint.toBuffer(), provider.wallet.publicKey.toBuffer()],
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
    })
});