import * as anchor from "@anchor-lang/core";
import { Program } from "@anchor-lang/core";
import { Amm } from "../target/types/amm";
import { Keypair, LAMPORTS_PER_SOL, PublicKey, SystemProgram } from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  createMint,
  getAssociatedTokenAddressSync,
  getOrCreateAssociatedTokenAccount,
  mintTo,
  getAccount,
  getMint,
} from "@solana/spl-token";
import { BN } from "bn.js";
import { expect } from "chai";

const DECIMALS = 6;
const ONE = 1_000_000n; // one whole token

const FEE = 30; // bps, stays in the pool
const PROTOCOL_FEE = 10; // bps, goes to the treasury

// same math as curve.rs, in bigint, so expected values are computed not typed in
const mulDiv = (a: bigint, b: bigint, c: bigint) => (a * b) / c;
const mulDivUp = (a: bigint, b: bigint, c: bigint) => (a * b + c - 1n) / c;
const swapOut = (net: bigint, rIn: bigint, rOut: bigint) => (rOut * net) / (rIn + net);
const feeAmt = (amt: bigint, bps: number) => (amt * BigInt(bps)) / 10_000n;

describe("amm", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.amm as Program<Amm>;
  const connection = provider.connection;
  const payer = (provider.wallet as any).payer as Keypair;

  const admin = Keypair.generate();
  const lp = Keypair.generate();
  const trader = Keypair.generate();

  let mintX: PublicKey;
  let mintY: PublicKey;

  const bn = (n: bigint) => new BN(n.toString());
  const bal = async (ata: PublicKey) => (await getAccount(connection, ata)).amount;
  const ata = (mint: PublicKey, owner: PublicKey) =>
    getAssociatedTokenAddressSync(mint, owner, true);

  const fund = async (kp: Keypair, sol: number) => {
    const sig = await connection.requestAirdrop(kp.publicKey, sol * LAMPORTS_PER_SOL);
    const bh = await connection.getLatestBlockhash();
    await connection.confirmTransaction({ signature: sig, ...bh }, "confirmed");
  };

  // every address a pool needs, derived from its seed
  const pdas = (seed: number) => {
    const config = PublicKey.findProgramAddressSync(
      [Buffer.from("config"), new BN(seed).toArrayLike(Buffer, "le", 8)],
      program.programId
    )[0];
    const mintLp = PublicKey.findProgramAddressSync(
      [Buffer.from("lp"), config.toBuffer()],
      program.programId
    )[0];
    const treasury = PublicKey.findProgramAddressSync(
      [Buffer.from("treasury"), config.toBuffer()],
      program.programId
    )[0];
    return {
      config,
      mintLp,
      treasury,
      vaultX: ata(mintX, config),
      vaultY: ata(mintY, config),
      treasuryX: ata(mintX, treasury),
      treasuryY: ata(mintY, treasury),
    };
  };

  const initialize = (seed: number, fee: number, protocolFee: number, mintYOverride?: PublicKey) => {
    const p = pdas(seed);
    const my = mintYOverride ?? mintY;
    return program.methods
      .initialize(new BN(seed), fee, protocolFee)
      .accountsStrict({
        initializer: admin.publicKey,
        mintX,
        mintY: my,
        config: p.config,
        mintLp: p.mintLp,
        treasury: p.treasury,
        vaultX: p.vaultX,
        vaultY: ata(my, p.config),
        treasuryX: p.treasuryX,
        treasuryY: ata(my, p.treasury),
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .signers([admin])
      .rpc();
  };

  // the main pool every test below shares
  const POOL = 1;
  const pool = () => pdas(POOL);

  const deposit = (user: Keypair, amount: bigint, maxX: bigint, maxY: bigint) => {
    const p = pool();
    return program.methods
      .deposit(bn(amount), bn(maxX), bn(maxY))
      .accountsStrict({
        user: user.publicKey,
        mintX,
        mintY,
        config: p.config,
        mintLp: p.mintLp,
        vaultX: p.vaultX,
        vaultY: p.vaultY,
        userX: ata(mintX, user.publicKey),
        userY: ata(mintY, user.publicKey),
        userLp: ata(p.mintLp, user.publicKey),
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .signers([user])
      .rpc();
  };

  const withdraw = (user: Keypair, amount: bigint, minX: bigint, minY: bigint) => {
    const p = pool();
    return program.methods
      .withdraw(bn(amount), bn(minX), bn(minY))
      .accountsStrict({
        user: user.publicKey,
        mintX,
        mintY,
        config: p.config,
        mintLp: p.mintLp,
        vaultX: p.vaultX,
        vaultY: p.vaultY,
        userX: ata(mintX, user.publicKey),
        userY: ata(mintY, user.publicKey),
        userLp: ata(p.mintLp, user.publicKey),
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .signers([user])
      .rpc();
  };

  const swap = (user: Keypair, isX: boolean, amountIn: bigint, minOut: bigint) => {
    const p = pool();
    return program.methods
      .swap(isX, bn(amountIn), bn(minOut))
      .accountsStrict({
        user: user.publicKey,
        mintX,
        mintY,
        config: p.config,
        treasury: p.treasury,
        vaultX: p.vaultX,
        vaultY: p.vaultY,
        treasuryX: p.treasuryX,
        treasuryY: p.treasuryY,
        userX: ata(mintX, user.publicKey),
        userY: ata(mintY, user.publicKey),
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .signers([user])
      .rpc();
  };

  const setLocked = (signer: Keypair, locked: boolean) => {
    const accounts = { authority: signer.publicKey, config: pool().config };
    const m = locked ? program.methods.lock() : program.methods.unlock();
    return m.accountsStrict(accounts).signers([signer]).rpc();
  };

  const collectFees = (signer: Keypair) => {
    const p = pool();
    return program.methods
      .collectFees()
      .accountsStrict({
        authority: signer.publicKey,
        mintX,
        mintY,
        config: p.config,
        treasury: p.treasury,
        treasuryX: p.treasuryX,
        treasuryY: p.treasuryY,
        authorityX: ata(mintX, signer.publicKey),
        authorityY: ata(mintY, signer.publicKey),
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .signers([signer])
      .rpc();
  };

  // what the next swap should pay out, read straight off the vaults
  const quote = async (isX: boolean, amountIn: bigint) => {
    const p = pool();
    const rx = await bal(p.vaultX);
    const ry = await bal(p.vaultY);
    const net = amountIn - feeAmt(amountIn, FEE) - feeAmt(amountIn, PROTOCOL_FEE);
    return isX ? swapOut(net, rx, ry) : swapOut(net, ry, rx);
  };

  before(async () => {
    await fund(admin, 10);
    await fund(lp, 10);
    await fund(trader, 10);

    mintX = await createMint(connection, payer, payer.publicKey, null, DECIMALS);
    mintY = await createMint(connection, payer, payer.publicKey, null, DECIMALS);

    for (const kp of [lp, trader]) {
      const x = (await getOrCreateAssociatedTokenAccount(connection, payer, mintX, kp.publicKey)).address;
      const y = (await getOrCreateAssociatedTokenAccount(connection, payer, mintY, kp.publicKey)).address;
      await mintTo(connection, payer, mintX, x, payer, 1000 * Number(ONE));
      await mintTo(connection, payer, mintY, y, payer, 1000 * Number(ONE));
    }
  });

  it("initialize stores the config and makes empty vaults", async () => {
    await initialize(POOL, FEE, PROTOCOL_FEE);
    const p = pool();

    const cfg = await program.account.config.fetch(p.config);
    expect(cfg.authority.toBase58()).to.equal(admin.publicKey.toBase58());
    expect(cfg.mintX.toBase58()).to.equal(mintX.toBase58());
    expect(cfg.mintY.toBase58()).to.equal(mintY.toBase58());
    expect(cfg.fee).to.equal(FEE);
    expect(cfg.protocolFee).to.equal(PROTOCOL_FEE);
    expect(cfg.locked).to.equal(false);

    expect(await bal(p.vaultX)).to.equal(0n);
    expect(await bal(p.vaultY)).to.equal(0n);
    expect(await bal(p.treasuryX)).to.equal(0n);
    expect(await bal(p.treasuryY)).to.equal(0n);

    const lpMint = await getMint(connection, p.mintLp);
    expect(lpMint.supply).to.equal(0n);
    expect(lpMint.mintAuthority.toBase58()).to.equal(p.config.toBase58());
  });

  it("wont initialize with the same mint on both sides", async () => {
    try {
      await initialize(2, FEE, PROTOCOL_FEE, mintX);
      expect.fail("made a pool of x and x");
    } catch (err) {
      // not our error. both vaults derive to one ATA and the second init is what blows up
      expect(err.toString()).to.match(/owner is not allowed|already in use/);
    }
    expect(await connection.getAccountInfo(pdas(2).config)).to.equal(null);
  });

  it("fees adding up to 9999 bps are allowed", async () => {
    await initialize(3, 9_000, 999);
    const cfg = await program.account.config.fetch(pdas(3).config);
    expect(cfg.fee + cfg.protocolFee).to.equal(9_999);
  });

  it("fees adding up to 10000 bps are not", async () => {
    try {
      await initialize(4, 9_000, 1_000);
      expect.fail("made a pool that eats the whole swap");
    } catch (err) {
      // check is strict `<`, so exactly 100 percent is the first value that fails
      expect(err.toString()).to.include("InvalidFee");
    }
  });

  it("first deposit sets the ratio and mints exactly what was asked", async () => {
    const p = pool();
    await deposit(lp, 100n * ONE, 100n * ONE, 200n * ONE);

    expect(await bal(p.vaultX)).to.equal(100n * ONE);
    expect(await bal(p.vaultY)).to.equal(200n * ONE);
    expect(await bal(ata(p.mintLp, lp.publicKey))).to.equal(100n * ONE);
  });

  it("second deposit is proportional to the pool", async () => {
    const p = pool();
    const rx = await bal(p.vaultX);
    const ry = await bal(p.vaultY);
    const supply = (await getMint(connection, p.mintLp)).supply;

    const want = 10n * ONE;
    const expectX = mulDivUp(want, rx, supply);
    const expectY = mulDivUp(want, ry, supply);

    await deposit(lp, want, expectX, expectY);

    expect(await bal(p.vaultX)).to.equal(rx + expectX);
    expect(await bal(p.vaultY)).to.equal(ry + expectY);
    expect(await bal(ata(p.mintLp, lp.publicKey))).to.equal(110n * ONE);
  });

  it("deposit of zero fails", async () => {
    try {
      await deposit(lp, 0n, ONE, ONE);
      expect.fail("deposited nothing");
    } catch (err) {
      expect(err.toString()).to.include("InvalidAmount");
    }
  });

  it("deposit fails when max_x is one under the real cost", async () => {
    const p = pool();
    const rx = await bal(p.vaultX);
    const ry = await bal(p.vaultY);
    const supply = (await getMint(connection, p.mintLp)).supply;
    const want = 5n * ONE;
    const costX = mulDivUp(want, rx, supply);
    const costY = mulDivUp(want, ry, supply);

    try {
      await deposit(lp, want, costX - 1n, costY);
      expect.fail("paid less than the pool ratio");
    } catch (err) {
      expect(err.toString()).to.include("SlippageExceeded");
    }
  });

  it("swap x for y pays the formula, lp fee stays in the vault, protocol cut hits the treasury", async () => {
    const p = pool();
    const amountIn = 10n * ONE;
    const rxBefore = await bal(p.vaultX);
    const ryBefore = await bal(p.vaultY);
    const traderYBefore = await bal(ata(mintY, trader.publicKey));

    const out = await quote(true, amountIn);
    await swap(trader, true, amountIn, 0n);

    const protocolCut = feeAmt(amountIn, PROTOCOL_FEE);
    expect(await bal(ata(mintY, trader.publicKey))).to.equal(traderYBefore + out);
    expect(await bal(p.vaultY)).to.equal(ryBefore - out);
    // everything but the protocol cut lands in the vault, so the lp fee is in there
    expect(await bal(p.vaultX)).to.equal(rxBefore + amountIn - protocolCut);
    expect(await bal(p.treasuryX)).to.equal(protocolCut);
  });

  it("swap y for x and k never drops", async () => {
    const p = pool();
    const kBefore = (await bal(p.vaultX)) * (await bal(p.vaultY));
    const traderXBefore = await bal(ata(mintX, trader.publicKey));

    const amountIn = 7n * ONE;
    const out = await quote(false, amountIn);
    await swap(trader, false, amountIn, 0n);

    expect(await bal(ata(mintX, trader.publicKey))).to.equal(traderXBefore + out);
    expect(await bal(p.treasuryY)).to.equal(feeAmt(amountIn, PROTOCOL_FEE));

    const kAfter = (await bal(p.vaultX)) * (await bal(p.vaultY));
    expect(kAfter >= kBefore).to.equal(true);
  });

  it("swap fails when min_out is one over the real output", async () => {
    const out = await quote(true, ONE);
    try {
      await swap(trader, true, ONE, out + 1n);
      expect.fail("got more than the pool can give");
    } catch (err) {
      expect(err.toString()).to.include("SlippageExceeded");
    }
  });

  it("swap with min_out exactly at the real output passes", async () => {
    const out = await quote(true, ONE);
    const before = await bal(ata(mintY, trader.publicKey));
    // check is `>=`, so the exact quote is the worst fill that still goes through
    await swap(trader, true, ONE, out);
    expect(await bal(ata(mintY, trader.publicKey))).to.equal(before + out);
  });

  it("deposit after a swap rounds up against the depositor", async () => {
    const p = pool();
    const rx = await bal(p.vaultX);
    const ry = await bal(p.vaultY);
    const supply = (await getMint(connection, p.mintLp)).supply;
    const want = 7n;

    const ceilX = mulDivUp(want, rx, supply);
    const ceilY = mulDivUp(want, ry, supply);
    // reserves are lopsided after the swaps so at least one of these isnt exact
    expect(ceilX > mulDiv(want, rx, supply) || ceilY > mulDiv(want, ry, supply)).to.equal(true);

    await deposit(lp, want, ceilX, ceilY);
    expect(await bal(p.vaultX)).to.equal(rx + ceilX);
    expect(await bal(p.vaultY)).to.equal(ry + ceilY);
  });

  it("withdraw burns lp and returns the proportional share", async () => {
    const p = pool();
    const rx = await bal(p.vaultX);
    const ry = await bal(p.vaultY);
    const supply = (await getMint(connection, p.mintLp)).supply;
    const lpAta = ata(p.mintLp, lp.publicKey);
    const lpBefore = await bal(lpAta);
    const xBefore = await bal(ata(mintX, lp.publicKey));
    const yBefore = await bal(ata(mintY, lp.publicKey));

    const burnAmt = 50n * ONE;
    const outX = mulDiv(burnAmt, rx, supply);
    const outY = mulDiv(burnAmt, ry, supply);

    await withdraw(lp, burnAmt, outX, outY);

    expect(await bal(lpAta)).to.equal(lpBefore - burnAmt);
    expect(await bal(ata(mintX, lp.publicKey))).to.equal(xBefore + outX);
    expect(await bal(ata(mintY, lp.publicKey))).to.equal(yBefore + outY);
    expect(await bal(p.vaultX)).to.equal(rx - outX);
    expect(await bal(p.vaultY)).to.equal(ry - outY);
  });

  it("wont withdraw more lp than you have", async () => {
    const have = await bal(ata(pool().mintLp, lp.publicKey));
    try {
      await withdraw(lp, have + 1n, 0n, 0n);
      expect.fail("burned lp tokens that dont exist");
    } catch (err) {
      // the token program throws this one, not us
      expect(err.toString()).to.match(/insufficient funds|0x1\b/);
    }
  });

  it("collect_fees moves the treasury to the authority", async () => {
    const p = pool();
    const tx = await bal(p.treasuryX);
    const ty = await bal(p.treasuryY);
    expect(tx > 0n && ty > 0n).to.equal(true);

    await collectFees(admin);

    expect(await bal(p.treasuryX)).to.equal(0n);
    expect(await bal(p.treasuryY)).to.equal(0n);
    expect(await bal(ata(mintX, admin.publicKey))).to.equal(tx);
    expect(await bal(ata(mintY, admin.publicKey))).to.equal(ty);
  });

  it("only the authority can collect fees", async () => {
    try {
      await collectFees(trader);
      expect.fail("trader took the fees");
    } catch (err) {
      expect(err.toString()).to.match(/ConstraintHasOne|AnchorError/);
    }
  });

  it("only the authority can lock", async () => {
    try {
      await setLocked(trader, true);
      expect.fail("trader locked the pool");
    } catch (err) {
      expect(err.toString()).to.match(/ConstraintHasOne|AnchorError/);
    }
    expect((await program.account.config.fetch(pool().config)).locked).to.equal(false);
  });

  it("lock blocks swap", async () => {
    await setLocked(admin, true);
    expect((await program.account.config.fetch(pool().config)).locked).to.equal(true);

    try {
      await swap(trader, true, ONE, 0n);
      expect.fail("swapped on a locked pool");
    } catch (err) {
      expect(err.toString()).to.include("PoolLocked");
    }
  });

  it("lock blocks deposit", async () => {
    try {
      await deposit(lp, ONE, 100n * ONE, 100n * ONE);
      expect.fail("deposited into a locked pool");
    } catch (err) {
      expect(err.toString()).to.include("PoolLocked");
    }
  });

  it("withdraw still works while locked", async () => {
    const p = pool();
    const lpAta = ata(p.mintLp, lp.publicKey);
    const before = await bal(lpAta);

    await withdraw(lp, ONE, 0n, 0n);
    expect(await bal(lpAta)).to.equal(before - ONE);
  });

  it("unlock and swaps work again", async () => {
    await setLocked(admin, false);
    const before = await bal(ata(mintY, trader.publicKey));
    const out = await quote(true, ONE);

    await swap(trader, true, ONE, out);
    expect(await bal(ata(mintY, trader.publicKey))).to.equal(before + out);
  });
});
